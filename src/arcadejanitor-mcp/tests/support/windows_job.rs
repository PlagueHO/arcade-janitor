use std::{
    io,
    mem::{size_of, zeroed},
    os::windows::{
        io::{AsRawHandle, FromRawHandle, OwnedHandle},
        process::CommandExt,
    },
    process::{Child, Command},
    thread,
    time::{Duration, Instant},
};

use anyhow::{Context, Result, bail, ensure};
use windows_sys::Win32::{
    Foundation::{HANDLE, INVALID_HANDLE_VALUE},
    System::{
        Diagnostics::ToolHelp::{
            CreateToolhelp32Snapshot, TH32CS_SNAPTHREAD, THREADENTRY32, Thread32First, Thread32Next,
        },
        JobObjects::{
            AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
            JOBOBJECT_BASIC_ACCOUNTING_INFORMATION, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
            JobObjectBasicAccountingInformation, JobObjectExtendedLimitInformation,
            QueryInformationJobObject, SetInformationJobObject, TerminateJobObject,
        },
        Threading::{CREATE_SUSPENDED, OpenThread, ResumeThread, THREAD_SUSPEND_RESUME},
    },
};

pub struct Job(OwnedHandle);

fn owned(handle: HANDLE) -> io::Result<OwnedHandle> {
    if handle.is_null() || handle == INVALID_HANDLE_VALUE {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: Each caller supplies a newly created, valid handle with sole ownership.
    Ok(unsafe { OwnedHandle::from_raw_handle(handle) })
}

impl Job {
    pub fn spawn(command: &mut Command) -> Result<(Child, Self)> {
        // SAFETY: Null arguments request an unnamed job with default security.
        let job = Self(owned(unsafe {
            CreateJobObjectW(std::ptr::null(), std::ptr::null())
        })?);
        // SAFETY: This C structure permits zero initialization.
        let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { zeroed() };
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        // SAFETY: The job handle and initialized buffer remain valid for this call.
        let configured = unsafe {
            SetInformationJobObject(
                job.0.as_raw_handle(),
                JobObjectExtendedLimitInformation,
                (&limits as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
                size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )
        };
        if configured == 0 {
            return Err(io::Error::last_os_error()).context("configure test process job");
        }

        // Suspend before assignment so even an immediate launcher cannot create an unowned child.
        let mut child = command.creation_flags(CREATE_SUSPENDED).spawn()?;
        let attach = || -> Result<()> {
            // SAFETY: Both handles remain valid and the child is still suspended.
            if unsafe { AssignProcessToJobObject(job.0.as_raw_handle(), child.as_raw_handle()) }
                == 0
            {
                return Err(io::Error::last_os_error()).context("assign test launcher to job");
            }
            resume(&child)
        };
        if let Err(error) = attach() {
            child.kill().context("stop suspended test launcher")?;
            super::wait(&mut child, super::CLEANUP_WAIT)?;
            return Err(error);
        }
        Ok((child, job))
    }

    pub fn stop(&self, timeout: Duration) -> Result<()> {
        // SAFETY: The owned job handle is valid; termination covers all remaining descendants.
        if unsafe { TerminateJobObject(self.0.as_raw_handle(), 1) } == 0 {
            return Err(io::Error::last_os_error()).context("terminate test process job");
        }
        let deadline = Instant::now() + timeout;
        loop {
            // SAFETY: This C structure permits zero initialization.
            let mut accounting: JOBOBJECT_BASIC_ACCOUNTING_INFORMATION = unsafe { zeroed() };
            // SAFETY: The writable buffer has the required size and lives through the call.
            if unsafe {
                QueryInformationJobObject(
                    self.0.as_raw_handle(),
                    JobObjectBasicAccountingInformation,
                    (&mut accounting as *mut JOBOBJECT_BASIC_ACCOUNTING_INFORMATION).cast(),
                    size_of::<JOBOBJECT_BASIC_ACCOUNTING_INFORMATION>() as u32,
                    std::ptr::null_mut(),
                )
            } == 0
            {
                return Err(io::Error::last_os_error()).context("check test job cleanup");
            }
            if accounting.ActiveProcesses == 0 {
                return Ok(());
            }
            ensure!(
                Instant::now() < deadline,
                "test job cleanup exceeded {timeout:?}"
            );
            thread::sleep(Duration::from_millis(20));
        }
    }
}

fn resume(child: &Child) -> Result<()> {
    // SAFETY: This creates an owned snapshot without borrowing any memory.
    let snapshot = owned(unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0) })?;
    // SAFETY: This C structure permits zero initialization and requires its size field.
    let mut entry: THREADENTRY32 = unsafe { zeroed() };
    entry.dwSize = size_of::<THREADENTRY32>() as u32;
    // SAFETY: The snapshot and writable entry buffer remain valid during enumeration.
    let mut found = unsafe { Thread32First(snapshot.as_raw_handle(), &mut entry) };
    while found != 0 {
        if entry.th32OwnerProcessID == child.id() {
            // SAFETY: The suspended launcher has one initial thread. Open only that thread.
            let thread =
                owned(unsafe { OpenThread(THREAD_SUSPEND_RESUME, 0, entry.th32ThreadID) })?;
            // SAFETY: The owned thread handle has suspend/resume access.
            if unsafe { ResumeThread(thread.as_raw_handle()) } == u32::MAX {
                return Err(io::Error::last_os_error()).context("resume test launcher");
            }
            return Ok(());
        }
        // SAFETY: The snapshot and writable entry buffer remain valid.
        found = unsafe { Thread32Next(snapshot.as_raw_handle(), &mut entry) };
    }
    bail!(
        "cannot find suspended test launcher thread for PID {}",
        child.id()
    )
}
