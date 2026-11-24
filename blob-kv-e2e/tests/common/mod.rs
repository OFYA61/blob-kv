//! Shared end-to-end test harness for the `blob-kv` server and CLI.

#![allow(dead_code)]

use std::{
    fmt,
    fs::{self, File},
    io::{self, Read, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    os::{
        fd::{AsRawFd, FromRawFd},
        unix::process::CommandExt,
    },
    path::{Path, PathBuf},
    process::{Child, Command, ExitStatus, Output, Stdio},
    thread,
    time::{Duration, Instant},
};

use assert_cmd::cargo::cargo_bin;
use predicates::prelude::Predicate;
use tempfile::TempDir;

const HOST: &str = "127.0.0.1";
const STARTUP_TIMEOUT: Duration = Duration::from_secs(10);
const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(10);
const CLI_TIMEOUT: Duration = Duration::from_secs(10);

pub struct TestServer {
    child: Option<Child>,
    address: SocketAddr,
    data_dir: TempDir,
    log_file: PathBuf,
    stdout_path: PathBuf,
    stderr_path: PathBuf,
}

impl TestServer {
    pub fn start() -> Self {
        let data_dir = TempDir::with_prefix("blob-kv-e2e-").expect("failed to create temp dir");
        let log_file = data_dir.path().join("store.log");
        Self::start_with_dir(data_dir, log_file)
    }

    pub fn start_with_log_file(log_file: impl Into<PathBuf>) -> Self {
        let data_dir = TempDir::with_prefix("blob-kv-e2e-").expect("failed to create temp dir");
        Self::start_with_dir(data_dir, log_file.into())
    }

    pub fn try_start_with_log_file(log_file: impl Into<PathBuf>) -> Result<Self, ServerStartError> {
        let data_dir = TempDir::with_prefix("blob-kv-e2e-").map_err(ServerStartError::Io)?;
        let mut server = Self::new(data_dir, log_file.into());
        server.launch()?;
        Ok(server)
    }

    fn start_with_dir(data_dir: TempDir, log_file: PathBuf) -> Self {
        let mut server = Self::new(data_dir, log_file);
        if let Err(err) = server.launch() {
            panic!("failed to start blob-kv-server: {err}");
        }
        server
    }

    fn new(data_dir: TempDir, log_file: PathBuf) -> Self {
        let stdout_path = data_dir.path().join("server.stdout.log");
        let stderr_path = data_dir.path().join("server.stderr.log");

        Self {
            child: None,
            address: SocketAddr::from(([127, 0, 0, 1], 0)),
            data_dir,
            log_file,
            stdout_path,
            stderr_path,
        }
    }

    pub fn address(&self) -> SocketAddr {
        self.address
    }

    pub fn log_file(&self) -> &Path {
        &self.log_file
    }

    pub fn data_dir(&self) -> &Path {
        self.data_dir.path()
    }

    pub fn stdout(&self) -> String {
        fs::read_to_string(&self.stdout_path).unwrap_or_default()
    }

    pub fn stderr(&self) -> String {
        fs::read_to_string(&self.stderr_path).unwrap_or_default()
    }

    pub fn stop(&mut self) {
        self.terminate();
    }

    pub fn restart(&mut self) {
        self.stop();
        if let Err(err) = self.launch() {
            panic!("failed to restart blob-kv-server: {err}");
        }
    }

    fn launch(&mut self) -> Result<(), ServerStartError> {
        self.stop();

        let port = free_port().map_err(ServerStartError::Io)?;
        self.address = SocketAddr::from(([127, 0, 0, 1], port));

        let stdout = File::create(&self.stdout_path).map_err(ServerStartError::Io)?;
        let stderr = File::create(&self.stderr_path).map_err(ServerStartError::Io)?;

        let child = Command::new(server_binary())
            .arg("--host")
            .arg(self.address.ip().to_string())
            .arg("--port")
            .arg(self.address.port().to_string())
            .arg("--log-file")
            .arg(&self.log_file)
            .current_dir(self.data_dir.path())
            .stdin(Stdio::null())
            .stdout(Stdio::from(stdout))
            .stderr(Stdio::from(stderr))
            .spawn()
            .map_err(ServerStartError::Spawn)?;

        self.child = Some(child);

        if let Err(err) = self.wait_until_ready() {
            self.stop();
            return Err(err);
        }

        Ok(())
    }

    fn wait_until_ready(&mut self) -> Result<(), ServerStartError> {
        let deadline = Instant::now() + STARTUP_TIMEOUT;

        loop {
            if let Some(child) = self.child.as_mut() {
                match child.try_wait() {
                    Ok(Some(status)) => {
                        let (stdout, stderr) = (self.stdout(), self.stderr());
                        return Err(ServerStartError::Exited {
                            status,
                            stdout,
                            stderr,
                        });
                    }
                    Ok(None) => {}
                    Err(err) => return Err(ServerStartError::Io(err)),
                }
            }

            // The server has no explicit ready message, so poll the address
            // with a bounded timeout instead of sleeping for a fixed period.
            if TcpStream::connect_timeout(&self.address, Duration::from_millis(100)).is_ok() {
                return Ok(());
            }

            if Instant::now() >= deadline {
                let (stdout, stderr) = (self.stdout(), self.stderr());
                return Err(ServerStartError::Timeout {
                    timeout: STARTUP_TIMEOUT,
                    address: self.address,
                    stdout,
                    stderr,
                });
            }

            thread::sleep(Duration::from_millis(10));
        }
    }

    fn terminate(&mut self) -> Option<ExitStatus> {
        let mut child = self.child.take()?;

        match child.try_wait() {
            Ok(Some(status)) => return Some(status),
            Ok(None) => {}
            Err(_) => {}
        }

        // The server does not install a signal handler, so SIGTERM makes it
        // exit promptly and lets the OS flush its append-only log writes.
        unsafe {
            libc::kill(child.id() as libc::pid_t, libc::SIGTERM);
        }

        let deadline = Instant::now() + SHUTDOWN_TIMEOUT;
        loop {
            match child.try_wait() {
                Ok(Some(status)) => return Some(status),
                Ok(None) => {
                    if Instant::now() >= deadline {
                        let _ = child.kill();
                        return child.wait().ok();
                    }
                    thread::sleep(Duration::from_millis(10));
                }
                Err(_) => {
                    let _ = child.kill();
                    return child.wait().ok();
                }
            }
        }
    }
}

impl Drop for TestServer {
    fn drop(&mut self) {
        self.stop();
    }
}

#[derive(Debug)]
pub struct CommandResult {
    command: String,
    output: Output,
}

impl CommandResult {
    fn new(command: String, output: Output) -> Self {
        Self { command, output }
    }

    pub fn command(&self) -> &str {
        &self.command
    }

    pub fn output(&self) -> &Output {
        &self.output
    }

    pub fn status(&self) -> ExitStatus {
        self.output.status
    }

    pub fn code(&self) -> Option<i32> {
        self.output.status.code()
    }

    pub fn success(&self) -> bool {
        self.output.status.success()
    }

    pub fn raw_stdout(&self) -> &[u8] {
        &self.output.stdout
    }

    pub fn raw_stderr(&self) -> &[u8] {
        &self.output.stderr
    }

    pub fn stdout(&self) -> String {
        normalize_stdout(&self.output.stdout)
    }

    pub fn stderr(&self) -> String {
        normalize_stderr(&self.output.stderr)
    }

    #[track_caller]
    pub fn assert_success(&self) -> &Self {
        assert!(
            self.success(),
            "command {:?} failed with {}\n--- stdout ---\n{}\n--- stderr ---\n{}",
            self.command,
            self.status(),
            self.stdout(),
            self.stderr(),
        );
        self
    }

    #[track_caller]
    pub fn assert_failure(&self) -> &Self {
        assert!(
            !self.success(),
            "command {:?} unexpectedly succeeded\n--- stdout ---\n{}\n--- stderr ---\n{}",
            self.command,
            self.stdout(),
            self.stderr(),
        );
        self
    }

    #[track_caller]
    pub fn assert_code(&self, expected: i32) -> &Self {
        assert_eq!(
            self.code(),
            Some(expected),
            "command {:?} exited with an unexpected status\n--- stdout ---\n{}\n--- stderr ---\n{}",
            self.command,
            self.stdout(),
            self.stderr(),
        );
        self
    }

    #[track_caller]
    pub fn assert_stdout(&self, predicate: impl Predicate<str>) -> &Self {
        let stdout = self.stdout();
        assert!(
            predicate.eval(&stdout),
            "command {:?} stdout did not match the predicate\n--- stdout ---\n{stdout}\n--- stderr ---\n{}",
            self.command,
            self.stderr(),
        );
        self
    }

    #[track_caller]
    pub fn assert_stderr(&self, predicate: impl Predicate<str>) -> &Self {
        let stderr = self.stderr();
        assert!(
            predicate.eval(&stderr),
            "command {:?} stderr did not match the predicate\n--- stdout ---\n{}\n--- stderr ---\n{stderr}",
            self.command,
            self.stdout(),
        );
        self
    }

    #[track_caller]
    pub fn assert_stdout_contains(&self, needle: &str) -> &Self {
        self.assert_stdout(predicates::str::contains(needle))
    }

    #[track_caller]
    pub fn assert_stderr_contains(&self, needle: &str) -> &Self {
        self.assert_stderr(predicates::str::contains(needle))
    }
}

pub struct TestClient {
    address: SocketAddr,
}

impl TestClient {
    pub fn new(address: SocketAddr) -> Self {
        Self { address }
    }

    pub fn address(&self) -> SocketAddr {
        self.address
    }

    pub fn set(&self, key: &str, value: &str) -> CommandResult {
        self.run(format!(
            "SET {} {}",
            quote_argument(key),
            quote_argument(value)
        ))
    }

    pub fn get(&self, key: &str) -> CommandResult {
        self.run(format!("GET {}", quote_argument(key)))
    }

    pub fn del(&self, key: &str) -> CommandResult {
        self.run(format!("DEL {}", quote_argument(key)))
    }

    pub fn run(&self, command: impl AsRef<str>) -> CommandResult {
        let command = command.as_ref();
        self.execute(&[], Some(command))
    }

    pub fn run_with_args(&self, args: &[&str]) -> CommandResult {
        self.execute(args, None)
    }

    fn execute(&self, extra_args: &[&str], input: Option<&str>) -> CommandResult {
        let pty = Pty::open().expect("failed to open a pseudo-terminal");
        pty.set_window_size(24, 80)
            .expect("failed to set pseudo-terminal window size");
        pty.set_raw_mode()
            .expect("failed to put pseudo-terminal in raw mode");

        let mut command = Command::new(cli_binary());
        command
            .arg("--host")
            .arg(self.address.ip().to_string())
            .arg("--port")
            .arg(self.address.port().to_string())
            .args(extra_args)
            // Without a real terminal emulator, a dumb terminal prevents
            // reedline from waiting for cursor-position responses.
            .env("TERM", "dumb")
            .stdin(Stdio::from(pty.slave))
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        // Make the pseudo-terminal the child's controlling terminal. Reedline
        // switches the controlling terminal into raw mode and reads input from
        // it, while normal command output stays on the captured stdout/stderr
        // pipes.
        unsafe {
            command.pre_exec(|| {
                if libc::setsid() == -1 {
                    return Err(io::Error::last_os_error());
                }
                if libc::ioctl(libc::STDIN_FILENO, libc::TIOCSCTTY as _, 0) == -1 {
                    return Err(io::Error::last_os_error());
                }
                Ok(())
            });
        }

        let child = command.spawn().expect("failed to spawn blob-kv-cli");

        let mut input_bytes = Vec::new();
        if let Some(input) = input {
            input_bytes.extend_from_slice(input.as_bytes());
            input_bytes.push(b'\n');
        }
        // Ctrl-D makes reedline return Signal::CtrlD, which exits the REPL.
        input_bytes.push(0x04);

        let mut master = pty.master;
        // Ignore write errors: clap or a connection failure can make the CLI
        // exit before it ever reads stdin.
        let _ = master.write_all(&input_bytes);
        let _ = master.flush();

        let output =
            wait_with_output_timeout(child, CLI_TIMEOUT).expect("failed to wait for blob-kv-cli");

        drop(master);

        CommandResult::new(input.unwrap_or_default().to_string(), output)
    }
}

#[derive(Debug)]
pub enum ServerStartError {
    Io(io::Error),
    Spawn(io::Error),
    Exited {
        status: ExitStatus,
        stdout: String,
        stderr: String,
    },
    Timeout {
        timeout: Duration,
        address: SocketAddr,
        stdout: String,
        stderr: String,
    },
}

impl ServerStartError {
    pub fn status(&self) -> Option<ExitStatus> {
        match self {
            Self::Exited { status, .. } => Some(*status),
            _ => None,
        }
    }

    pub fn stdout(&self) -> &str {
        match self {
            Self::Exited { stdout, .. } | Self::Timeout { stdout, .. } => stdout,
            _ => "",
        }
    }

    pub fn stderr(&self) -> &str {
        match self {
            Self::Exited { stderr, .. } | Self::Timeout { stderr, .. } => stderr,
            _ => "",
        }
    }
}

impl fmt::Display for ServerStartError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(err) | Self::Spawn(err) => write!(f, "{err}"),
            Self::Exited {
                status,
                stdout,
                stderr,
            } => write!(
                f,
                "server exited before becoming ready with {status}\n--- stdout ---\n{stdout}\n--- stderr ---\n{stderr}"
            ),
            Self::Timeout {
                timeout,
                address,
                stdout,
                stderr,
            } => write!(
                f,
                "server did not become ready at {address} within {timeout:?}\n--- stdout ---\n{stdout}\n--- stderr ---\n{stderr}"
            ),
        }
    }
}

impl std::error::Error for ServerStartError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(err) | Self::Spawn(err) => Some(err),
            _ => None,
        }
    }
}

struct Pty {
    master: File,
    slave: File,
}

impl Pty {
    fn open() -> io::Result<Self> {
        let master_fd = unsafe { libc::posix_openpt(libc::O_RDWR | libc::O_NOCTTY) };
        if master_fd == -1 {
            return Err(io::Error::last_os_error());
        }
        let master = unsafe { File::from_raw_fd(master_fd) };

        if unsafe { libc::grantpt(master_fd) } == -1 {
            return Err(io::Error::last_os_error());
        }
        if unsafe { libc::unlockpt(master_fd) } == -1 {
            return Err(io::Error::last_os_error());
        }

        let mut name = [0 as libc::c_char; 256];
        let result = unsafe { libc::ptsname_r(master_fd, name.as_mut_ptr(), name.len()) };
        if result != 0 {
            return Err(io::Error::from_raw_os_error(result));
        }

        let slave_fd = unsafe { libc::open(name.as_ptr(), libc::O_RDWR | libc::O_NOCTTY) };
        if slave_fd == -1 {
            return Err(io::Error::last_os_error());
        }
        let slave = unsafe { File::from_raw_fd(slave_fd) };

        Ok(Self { master, slave })
    }

    fn set_window_size(&self, rows: u16, columns: u16) -> io::Result<()> {
        let size = libc::winsize {
            ws_row: rows,
            ws_col: columns,
            ws_xpixel: 0,
            ws_ypixel: 0,
        };

        if unsafe { libc::ioctl(self.master.as_raw_fd(), libc::TIOCSWINSZ, &size) } == -1 {
            return Err(io::Error::last_os_error());
        }

        Ok(())
    }

    fn set_raw_mode(&self) -> io::Result<()> {
        let fd = self.slave.as_raw_fd();
        let mut termios = std::mem::MaybeUninit::<libc::termios>::uninit();

        if unsafe { libc::tcgetattr(fd, termios.as_mut_ptr()) } == -1 {
            return Err(io::Error::last_os_error());
        }

        let mut termios = unsafe { termios.assume_init() };
        unsafe { libc::cfmakeraw(&mut termios) };

        if unsafe { libc::tcsetattr(fd, libc::TCSANOW, &termios) } == -1 {
            return Err(io::Error::last_os_error());
        }

        Ok(())
    }
}

fn free_port() -> io::Result<u16> {
    let listener = TcpListener::bind((HOST, 0))?;
    let port = listener.local_addr()?.port();
    drop(listener);
    Ok(port)
}

fn server_binary() -> PathBuf {
    binary_path("blob-kv-server")
}

fn cli_binary() -> PathBuf {
    binary_path("blob-kv-cli")
}

// Resolve a binary built from this package. Cargo sets CARGO_BIN_EXE_<name>
// for integration tests; the fallbacks cover custom target directories.
fn binary_path(name: &str) -> PathBuf {
    let env_name = format!("CARGO_BIN_EXE_{name}");
    if let Some(path) = std::env::var_os(&env_name) {
        return PathBuf::from(path);
    }

    let file_name = format!("{name}{}", std::env::consts::EXE_SUFFIX);

    if let Some(target_dir) = std::env::var_os("CARGO_TARGET_DIR") {
        let candidate = PathBuf::from(target_dir).join(&file_name);
        if candidate.exists() {
            return candidate;
        }
    }

    let profile = if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    };
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let candidate = manifest_dir
        .join("..")
        .join("target")
        .join(profile)
        .join(&file_name);
    if candidate.exists() {
        return candidate;
    }

    cargo_bin(name)
}

fn wait_with_output_timeout(mut child: Child, timeout: Duration) -> io::Result<Output> {
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();

    let stdout_reader = stdout.map(|mut stdout| {
        thread::spawn(move || {
            let mut buffer = Vec::new();
            let _ = stdout.read_to_end(&mut buffer);
            buffer
        })
    });
    let stderr_reader = stderr.map(|mut stderr| {
        thread::spawn(move || {
            let mut buffer = Vec::new();
            let _ = stderr.read_to_end(&mut buffer);
            buffer
        })
    });

    let deadline = Instant::now() + timeout;
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }

        if Instant::now() >= deadline {
            let _ = child.kill();
            break child.wait()?;
        }

        thread::sleep(Duration::from_millis(5));
    };

    let stdout = stdout_reader
        .map(|reader| reader.join().unwrap_or_default())
        .unwrap_or_default();
    let stderr = stderr_reader
        .map(|reader| reader.join().unwrap_or_default())
        .unwrap_or_default();

    Ok(Output {
        status,
        stdout,
        stderr,
    })
}

fn quote_argument(argument: &str) -> String {
    let needs_quotes = argument.is_empty()
        || argument
            .chars()
            .any(|character| character.is_whitespace() || character == '"' || character == '\'');

    if !needs_quotes {
        return argument.to_string();
    }

    if !argument.contains('\'') {
        format!("'{argument}'")
    } else if !argument.contains('"') {
        format!("\"{argument}\"")
    } else {
        panic!("cannot represent an argument containing both quote characters");
    }
}

fn normalize_stdout(bytes: &[u8]) -> String {
    let text = strip_ansi(&String::from_utf8_lossy(bytes));
    let text = text.replace("\r\n", "\n").replace('\r', "");
    let mut lines: Vec<&str> = text.split('\n').collect();

    while lines.last().is_some_and(|line| line.is_empty()) {
        lines.pop();
    }

    if lines.last().is_some_and(|line| line.trim() == "Aborted!") {
        lines.pop();
    }

    while lines.last().is_some_and(|line| line.is_empty()) {
        lines.pop();
    }

    lines.join("\n")
}

fn normalize_stderr(bytes: &[u8]) -> String {
    let text = strip_ansi(&String::from_utf8_lossy(bytes))
        .replace("\r\n", "\n")
        .replace('\r', "");

    // Reedline paints prompts and echoes the current input on stderr. Drop
    // those lines so the remaining text is the CLI's own diagnostics.
    text.lines()
        .filter(|line| {
            let trimmed = line.trim();
            !trimmed.is_empty() && !trimmed.starts_with('>')
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn strip_ansi(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();

    while let Some(character) = chars.next() {
        if character != '\x1b' {
            output.push(character);
            continue;
        }

        match chars.next() {
            // CSI sequence: parameters/intermediates followed by a final byte.
            Some('[') => {
                while let Some(next) = chars.next() {
                    if ('\x40'..='\x7e').contains(&next) {
                        break;
                    }
                }
            }
            // OSC sequence, terminated by BEL or ST.
            Some(']') => {
                while let Some(next) = chars.next() {
                    if next == '\x07' {
                        break;
                    }
                    if next == '\x1b' && chars.peek() == Some(&'\\') {
                        chars.next();
                        break;
                    }
                }
            }
            // Other two-byte escape sequences (for example save/restore).
            Some(_) | None => {}
        }
    }

    output
}
