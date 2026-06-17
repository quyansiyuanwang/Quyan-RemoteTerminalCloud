use anyhow::Result;

mod types;

#[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

pub use types::{
    LINUX_SYSTEMD_SERVICE_NAME, MACOS_PLIST_PATH, MACOS_SERVICE_LABEL, ServiceActionResult,
    WINDOWS_SERVICE_NAME,
};

pub fn service_status() -> ServiceActionResult {
    #[cfg(target_os = "windows")]
    {
        windows::service_status()
    }
    #[cfg(target_os = "macos")]
    {
        macos::service_status()
    }
    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    {
        linux::service_status()
    }
}

pub fn install_service(install_root: &str, _token: Option<&str>) -> Result<ServiceActionResult> {
    #[cfg(target_os = "windows")]
    {
        windows::install_service(install_root, _token)
    }
    #[cfg(target_os = "macos")]
    {
        macos::install_service(install_root)
    }
    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    {
        linux::install_service(install_root)
    }
}

pub fn uninstall_service(_install_root: &str) -> Result<ServiceActionResult> {
    #[cfg(target_os = "windows")]
    {
        let _ = _install_root;
        windows::uninstall_service()
    }
    #[cfg(target_os = "macos")]
    {
        macos::uninstall_service()
    }
    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    {
        let _ = _install_root;
        linux::uninstall_service()
    }
}

pub fn start_service() -> Result<ServiceActionResult> {
    #[cfg(target_os = "windows")]
    {
        windows::start_service()
    }
    #[cfg(target_os = "macos")]
    {
        macos::start_service()
    }
    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    {
        linux::start_service()
    }
}

pub fn stop_service(_install_root: &str) -> Result<ServiceActionResult> {
    #[cfg(target_os = "windows")]
    {
        let _ = _install_root;
        windows::stop_service()
    }
    #[cfg(target_os = "macos")]
    {
        macos::stop_service()
    }
    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    {
        let _ = _install_root;
        linux::stop_service()
    }
}

pub fn restart_service(_install_root: &str) -> Result<ServiceActionResult> {
    #[cfg(target_os = "windows")]
    {
        let _ = _install_root;
        windows::restart_service()
    }
    #[cfg(target_os = "macos")]
    {
        macos::restart_service()
    }
    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    {
        let _ = _install_root;
        linux::restart_service()
    }
}
