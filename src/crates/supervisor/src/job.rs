//! Windows Job Object used to guarantee child cleanup.

#[cfg(not(windows))]
#[derive(Debug, Default)]
pub struct JobHandle;

#[cfg(not(windows))]
impl JobHandle {
    pub fn new() -> std::io::Result<Self> {
        Ok(Self)
    }

    pub fn assign(&self, _child: &tokio::process::Child) -> std::io::Result<()> {
        Ok(())
    }
}

#[cfg(windows)]
mod windows_job {
    use std::io;
    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
    use windows_sys::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
        SetInformationJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
        JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    };

    pub struct JobHandle(HANDLE);

    // The handle is owned by the supervisor and used from async tasks.
    unsafe impl Send for JobHandle {}
    unsafe impl Sync for JobHandle {}

    impl JobHandle {
        pub fn new() -> io::Result<Self> {
            let job = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
            if job.is_null() {
                return Err(io::Error::last_os_error());
            }

            let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { std::mem::zeroed() };
            info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            let ok = unsafe {
                SetInformationJobObject(
                    job,
                    JobObjectExtendedLimitInformation,
                    &info as *const _ as *const _,
                    std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
                )
            };
            if ok == 0 {
                unsafe {
                    CloseHandle(job);
                }
                return Err(io::Error::last_os_error());
            }

            Ok(Self(job))
        }

        pub fn assign(&self, child: &tokio::process::Child) -> io::Result<()> {
            let Some(raw) = child.raw_handle() else {
                return Err(io::Error::other("processo sem handle"));
            };
            let handle = raw as HANDLE;
            let ok = unsafe { AssignProcessToJobObject(self.0, handle) };
            if ok == 0 {
                Err(io::Error::last_os_error())
            } else {
                Ok(())
            }
        }
    }

    impl Drop for JobHandle {
        fn drop(&mut self) {
            unsafe {
                CloseHandle(self.0);
            }
        }
    }
}

#[cfg(windows)]
pub use windows_job::JobHandle;
