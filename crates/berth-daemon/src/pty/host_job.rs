use std::sync::{Mutex, OnceLock};

use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;
use windows_sys::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
};
use windows_sys::Win32::System::JobObjects::{
    AssignProcessToJobObject, IsProcessInJob, TerminateJobObject,
};
use windows_sys::Win32::System::Threading::{
    GetCurrentProcessId, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SET_QUOTA,
    PROCESS_TERMINATE,
};

use super::kill::{kill_on_close_job, Handle};

const CONSOLE_HOSTS: [&str; 2] = ["OpenConsole.exe", "conhost.exe"];

static JOB_THE_HOSTS_DIE_WITH: OnceLock<Option<Handle>> = OnceLock::new();
static ADOPTING: Mutex<()> = Mutex::new(());

pub(super) struct HostsBeforeOpenpty(Vec<u32>);

pub(super) fn hosts_before_openpty() -> HostsBeforeOpenpty {
    HostsBeforeOpenpty(console_host_children())
}

pub(super) fn tie_new_hosts_to_this_process(before: HostsBeforeOpenpty) {
    let Some(job) = JOB_THE_HOSTS_DIE_WITH.get_or_init(kill_on_close_job) else {
        return;
    };
    let _one_at_a_time = ADOPTING.lock().unwrap_or_else(|e| e.into_inner());
    for pid in console_host_children() {
        if !before.0.contains(&pid) {
            adopt(job, pid);
        }
    }
}

pub(super) fn kill_every_host() {
    if let Some(Some(job)) = JOB_THE_HOSTS_DIE_WITH.get() {
        unsafe { TerminateJobObject(job.0, 1) };
    }
}

fn console_host_children() -> Vec<u32> {
    let own_pid = unsafe { GetCurrentProcessId() };
    let mut hosts = Vec::new();
    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snapshot == INVALID_HANDLE_VALUE {
            tracing::warn!(
                error = %std::io::Error::last_os_error(),
                "process snapshot failed — console hosts stay outside the job"
            );
            return hosts;
        }
        let snapshot = Handle(snapshot);
        let mut entry: PROCESSENTRY32W = std::mem::zeroed();
        entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
        let mut more = Process32FirstW(snapshot.0, &mut entry) != 0;
        while more {
            if entry.th32ParentProcessID == own_pid && is_console_host(&entry.szExeFile) {
                hosts.push(entry.th32ProcessID);
            }
            more = Process32NextW(snapshot.0, &mut entry) != 0;
        }
    }
    hosts
}

fn is_console_host(exe: &[u16]) -> bool {
    let len = exe.iter().position(|&c| c == 0).unwrap_or(exe.len());
    let name = String::from_utf16_lossy(&exe[..len]);
    CONSOLE_HOSTS
        .iter()
        .any(|host| name.eq_ignore_ascii_case(host))
}

fn adopt(job: &Handle, pid: u32) {
    unsafe {
        let host = OpenProcess(
            PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SET_QUOTA | PROCESS_TERMINATE,
            0,
            pid,
        );
        if host.is_null() {
            tracing::warn!(pid, error = %std::io::Error::last_os_error(), "OpenProcess on a console host failed");
            return;
        }
        let host = Handle(host);
        let mut in_job = 0;
        if IsProcessInJob(host.0, job.0, &mut in_job) != 0 && in_job != 0 {
            return;
        }
        if AssignProcessToJobObject(job.0, host.0) == 0 {
            tracing::warn!(pid, error = %std::io::Error::last_os_error(), "AssignProcessToJobObject on a console host failed");
        }
    }
}
