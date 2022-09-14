use std::ffi::OsString;
use nix::sys::wait::{WaitPidFlag, WaitStatus};
use nix::unistd::Pid;
use nix::sys::signal::Signal;
use std::process::Command;
use std::os::unix::process::CommandExt;
use anyhow::{anyhow, Result};
use log::debug;

pub struct Credentials {
    pub uid: u32,
    pub gid: u32,
}

pub enum ExitStatus {
    Exited(i32),
    Signaled(Signal),
}

// runs the child and reaps all of its children as well
pub fn run_child(argv: &[OsString], creds: &Credentials) -> Result<ExitStatus> {
    // Don't use tokio::process::Command because it wants to reap the process.
    // However we need to run waitpid() ourselves to reap the zombies and it'll
    // end up picking up the spawned child as well.
    let mut child = Command::new(&argv[0])
        .args(&argv[1..])
        .uid(creds.uid)
        .gid(creds.gid)
        .process_group(0)
        .spawn()?;

    debug!("Child process started");
    let child_pid = Pid::from_raw(child.id() as i32);

    reap(child_pid)
}

// Reap processes until a process with sentinel pid exits.
// Returns the exit status for the sentinel process
fn reap(sentinel: Pid) -> Result<ExitStatus> {
    let flags = WaitPidFlag::empty();

    loop {
        let wait_status = nix::sys::wait::waitpid(None, Some(flags))
            .map_err(|e| anyhow!("waitpid failed: {}", e))?;

        match wait_status {
            WaitStatus::Exited(pid, status) => {
                debug!("Zombie with PID {} reaped", pid);
                if pid == sentinel {
                    // our child is done, exit
                    return Ok(ExitStatus::Exited(status));
                }
            },
            WaitStatus::Signaled(pid, sig, _) => {
                debug!("Zombie with PID {} reaped", pid);
                if pid == sentinel {
                    // our child crashed by signal, exit
                    return Ok(ExitStatus::Signaled(sig));
                }
            },
            _ => {}
        }
    }
}
