mod mount;
mod namespace;
mod security;

use crate::config::Config;
use crate::error::{Error, Result};
use nix::sys::wait::{waitpid, WaitStatus};
use nix::unistd::{fork, ForkResult, Pid};
use std::ffi::CString;
use std::path::{Path, PathBuf};

/// Sandbox configuration builder
#[derive(Debug, Clone)]
pub struct SandboxBuilder {
    /// Root directory of the sandbox (the worktree)
    root: PathBuf,
    /// Whether to share network with host
    share_net: bool,
    /// Additional read-only bind mounts
    ro_binds: Vec<(PathBuf, PathBuf)>,
    /// Additional read-write bind mounts
    rw_binds: Vec<(PathBuf, PathBuf)>,
    /// Environment variables to set
    env: Vec<(String, String)>,
    /// Working directory inside sandbox
    workdir: PathBuf,
}

impl SandboxBuilder {
    /// Create a new sandbox builder with the given root
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            share_net: true,
            ro_binds: vec![],
            rw_binds: vec![],
            env: vec![],
            workdir: PathBuf::from("/"),
        }
    }

    /// Configure from a Config object
    pub fn with_config(mut self, config: &Config) -> Self {
        self.share_net = config.network_enabled;

        // Add extra binds from config
        for path in &config.extra_ro_binds {
            self.ro_binds.push((path.clone(), path.clone()));
        }
        for path in &config.extra_rw_binds {
            self.rw_binds.push((path.clone(), path.clone()));
        }

        // Pass through environment variables
        for var in &config.env_passthrough {
            if let Ok(value) = std::env::var(var) {
                self.env.push((var.clone(), value));
            }
        }

        self
    }

    /// Set whether to share network
    #[allow(dead_code)]
    pub fn share_net(mut self, share: bool) -> Self {
        self.share_net = share;
        self
    }

    /// Add a read-only bind mount
    #[allow(dead_code)]
    pub fn ro_bind(mut self, src: impl Into<PathBuf>, dst: impl Into<PathBuf>) -> Self {
        self.ro_binds.push((src.into(), dst.into()));
        self
    }

    /// Add a read-write bind mount
    #[allow(dead_code)]
    pub fn rw_bind(mut self, src: impl Into<PathBuf>, dst: impl Into<PathBuf>) -> Self {
        self.rw_binds.push((src.into(), dst.into()));
        self
    }

    /// Set an environment variable
    pub fn env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.env.push((key.into(), value.into()));
        self
    }

    /// Set working directory
    pub fn workdir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.workdir = dir.into();
        self
    }

    /// Build the sandbox
    pub fn build(self) -> Sandbox {
        Sandbox {
            root: self.root,
            share_net: self.share_net,
            ro_binds: self.ro_binds,
            rw_binds: self.rw_binds,
            env: self.env,
            workdir: self.workdir,
        }
    }
}

/// A configured sandbox ready to run
#[derive(Debug)]
pub struct Sandbox {
    root: PathBuf,
    share_net: bool,
    ro_binds: Vec<(PathBuf, PathBuf)>,
    rw_binds: Vec<(PathBuf, PathBuf)>,
    env: Vec<(String, String)>,
    workdir: PathBuf,
}

impl Sandbox {
    /// Run an interactive shell in the sandbox
    pub fn enter(&self, shell: &str) -> Result<i32> {
        self.run_command(&[shell])
    }

    /// Run a command in the sandbox
    pub fn run(&self, command: &[String]) -> Result<i32> {
        let args: Vec<&str> = command.iter().map(|s| s.as_str()).collect();
        self.run_command(&args)
    }

    /// Internal: run a command in the sandbox
    fn run_command(&self, args: &[&str]) -> Result<i32> {
        if args.is_empty() {
            return Err(Error::SandboxSetup("no command specified".to_string()));
        }

        // Fork the process
        match unsafe { fork() } {
            Ok(ForkResult::Parent { child }) => {
                // Parent: wait for child
                self.wait_for_child(child)
            }
            Ok(ForkResult::Child) => {
                // Child: set up sandbox and exec
                if let Err(e) = self.setup_and_exec(args) {
                    eprintln!("sandbox setup failed: {e}");
                    std::process::exit(126);
                }
                unreachable!()
            }
            Err(e) => Err(Error::Nix(e)),
        }
    }

    /// Wait for child process and return exit code
    fn wait_for_child(&self, child: Pid) -> Result<i32> {
        loop {
            match waitpid(child, None) {
                Ok(WaitStatus::Exited(_, code)) => return Ok(code),
                Ok(WaitStatus::Signaled(_, sig, _)) => {
                    // Process killed by signal
                    return Ok(128 + sig as i32);
                }
                Ok(_) => continue, // Other status, keep waiting
                Err(nix::Error::EINTR) => continue, // Interrupted, retry
                Err(e) => return Err(Error::Nix(e)),
            }
        }
    }

    /// Set up the sandbox and exec the command (runs in child process)
    fn setup_and_exec(&self, args: &[&str]) -> Result<()> {
        // Step 1: Create user namespace and set up UID/GID mapping
        namespace::setup_user_namespace()?;

        // Step 2: Create other namespaces
        namespace::setup_other_namespaces(self.share_net)?;

        // Step 3: Set up mount namespace with filesystem
        self.setup_filesystem()?;

        // Step 4: Apply security hardening
        security::apply_security_restrictions()?;

        // Step 5: Change to working directory
        std::env::set_current_dir(&self.workdir)?;

        // Step 6: Set up environment
        // Clear environment first for security
        for (key, _) in std::env::vars() {
            std::env::remove_var(&key);
        }

        // Set required environment
        std::env::set_var("HOME", "/home/user");
        std::env::set_var("USER", "user");
        std::env::set_var("PATH", "/usr/local/bin:/usr/bin:/bin:/usr/local/sbin:/usr/sbin:/sbin");
        std::env::set_var("ROBOJAIL", "1");

        // Set user-specified environment
        for (key, value) in &self.env {
            std::env::set_var(key, value);
        }

        // Step 7: Exec the command
        let program = CString::new(args[0]).map_err(|e| {
            Error::SandboxSetup(format!("invalid command: {e}"))
        })?;

        let c_args: Vec<CString> = args
            .iter()
            .map(|s| CString::new(*s))
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| Error::SandboxSetup(format!("invalid argument: {e}")))?;

        // Use execvp to search PATH
        nix::unistd::execvp(&program, &c_args)?;

        unreachable!()
    }

    /// Set up the sandbox filesystem
    fn setup_filesystem(&self) -> Result<()> {
        // Make all mounts private first
        mount::make_mounts_private()?;

        // The root of our sandbox is a bind mount of the worktree
        // But we need to overlay system directories on top

        // Create a new root using a tmpfs where we'll build the filesystem
        let new_root = Path::new("/tmp/robojail-root");
        std::fs::create_dir_all(new_root)?;
        mount::mount_tmpfs(new_root)?;

        // First, bind mount the worktree as the base (this becomes /)
        // We do this by copying the worktree contents' view into the tmpfs
        // Actually, we need to bind mount the worktree content at the root
        mount::bind_mount(&self.root, new_root, false)?;

        // Now overlay the system directories on top
        // These are read-only bind mounts

        // System directories
        let system_dirs = [
            "/usr",
            "/bin",
            "/lib",
            "/lib64",
            "/sbin",
        ];

        for dir in &system_dirs {
            let src = Path::new(dir);
            if src.exists() {
                let dst = new_root.join(dir.trim_start_matches('/'));
                std::fs::create_dir_all(&dst)?;
                mount::bind_mount(src, &dst, true)?;
            }
        }

        // Minimal /etc - only essential files
        let etc_dst = new_root.join("etc");
        std::fs::create_dir_all(&etc_dst)?;
        mount::mount_tmpfs(&etc_dst)?;

        // Copy essential etc files (but not passwd/group - we create our own)
        for file in &["resolv.conf", "hosts", "nsswitch.conf"] {
            let src = Path::new("/etc").join(file);
            let dst = etc_dst.join(file);
            if src.exists() {
                if let Ok(content) = std::fs::read(&src) {
                    let _ = std::fs::write(&dst, content);
                }
            }
        }

        // Create custom /etc/passwd with our jail user (UID 1000)
        let passwd_content = "root:x:0:0:root:/root:/bin/bash\nuser:x:1000:1000:Jail User:/home/user:/bin/bash\nnobody:x:65534:65534:Nobody:/:/usr/bin/nologin\n";
        let _ = std::fs::write(etc_dst.join("passwd"), passwd_content);

        // Create custom /etc/group with our jail group (GID 1000)
        let group_content = "root:x:0:\nuser:x:1000:\nnogroup:x:65534:\n";
        let _ = std::fs::write(etc_dst.join("group"), group_content);

        // Create home directory for the jail user
        let home_dst = new_root.join("home/user");
        std::fs::create_dir_all(&home_dst)?;

        // Bind mount /etc/ssl for TLS
        let ssl_src = Path::new("/etc/ssl");
        if ssl_src.exists() {
            let ssl_dst = etc_dst.join("ssl");
            std::fs::create_dir_all(&ssl_dst)?;
            mount::bind_mount(ssl_src, &ssl_dst, true)?;
        }

        // /etc/ca-certificates
        let ca_src = Path::new("/etc/ca-certificates");
        if ca_src.exists() {
            let ca_dst = etc_dst.join("ca-certificates");
            std::fs::create_dir_all(&ca_dst)?;
            mount::bind_mount(ca_src, &ca_dst, true)?;
        }

        // Mount /proc
        let proc_dst = new_root.join("proc");
        std::fs::create_dir_all(&proc_dst)?;
        mount::mount_proc(&proc_dst)?;

        // Mount /dev with minimal devices
        let dev_dst = new_root.join("dev");
        std::fs::create_dir_all(&dev_dst)?;
        mount::setup_dev(&dev_dst)?;

        // Mount /tmp
        let tmp_dst = new_root.join("tmp");
        std::fs::create_dir_all(&tmp_dst)?;
        mount::mount_tmpfs(&tmp_dst)?;

        // Additional read-only binds
        for (src, dst) in &self.ro_binds {
            if src.exists() {
                let full_dst = new_root.join(dst.strip_prefix("/").unwrap_or(dst));
                if src.is_dir() {
                    std::fs::create_dir_all(&full_dst)?;
                } else {
                    // For files, create parent directory and an empty file to mount over
                    if let Some(parent) = full_dst.parent() {
                        std::fs::create_dir_all(parent)?;
                    }
                    std::fs::write(&full_dst, "")?;
                }
                mount::bind_mount(src, &full_dst, true)?;
            }
        }

        // Additional read-write binds
        for (src, dst) in &self.rw_binds {
            if src.exists() {
                let full_dst = new_root.join(dst.strip_prefix("/").unwrap_or(dst));
                if src.is_dir() {
                    std::fs::create_dir_all(&full_dst)?;
                } else {
                    // For files, create parent directory and an empty file to mount over
                    if let Some(parent) = full_dst.parent() {
                        std::fs::create_dir_all(parent)?;
                    }
                    std::fs::write(&full_dst, "")?;
                }
                mount::bind_mount(src, &full_dst, false)?;
            }
        }

        // Pivot to new root
        mount::pivot_root(new_root)?;

        Ok(())
    }
}

/// Create a default sandbox for a jail
pub fn create_jail_sandbox(worktree_path: &Path, config: &Config, entrypoint: Option<&[String]>) -> Sandbox {
    let mut builder = SandboxBuilder::new(worktree_path)
        .with_config(config)
        .env("HOME", "/home/user")
        .env("USER", "user")
        .workdir("/");

    // If entrypoint is specified and not in a standard system path,
    // bind-mount it to make it accessible inside the jail
    if let Some(ep) = entrypoint {
        if let Some(cmd) = ep.first() {
            let ep_path = Path::new(cmd);
            // Check if the binary is outside standard system paths
            let in_system_path = ep_path.starts_with("/usr")
                || ep_path.starts_with("/bin")
                || ep_path.starts_with("/sbin")
                || ep_path.starts_with("/lib");

            if !in_system_path {
                // Bind mount the binary read-only at the same path inside the jail
                builder = builder.ro_bind(ep_path, ep_path);
            }
        }
    }

    builder.build()
}
