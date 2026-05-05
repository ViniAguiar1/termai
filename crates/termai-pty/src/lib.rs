use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use std::io::{Read, Write};

/// Reader half of the PTY, intended to be moved to a background thread.
pub struct PtyReader {
    inner: Box<dyn Read + Send>,
}

impl Read for PtyReader {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.inner.read(buf)
    }
}

/// Managed PTY session wrapping a shell process.
pub struct PtySession {
    writer: Box<dyn Write + Send>,
    reader: Option<PtyReader>,
    _child: Box<dyn portable_pty::Child + Send + Sync>,
}

impl PtySession {
    /// Spawn a new shell in a PTY with the given dimensions.
    pub fn spawn(cols: u16, rows: u16) -> anyhow::Result<Self> {
        let pty_system = native_pty_system();

        let pair = pty_system.openpty(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })?;

        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());
        let mut cmd = CommandBuilder::new(&shell);
        cmd.cwd(std::env::current_dir().unwrap_or_else(|_| "/".into()));
        cmd.env("TERM", "xterm-256color");
        cmd.env("COLORTERM", "truecolor");

        let child = pair.slave.spawn_command(cmd)?;
        drop(pair.slave);

        let reader = pair.master.try_clone_reader()?;
        let writer = pair.master.take_writer()?;

        Ok(Self {
            writer,
            reader: Some(PtyReader { inner: reader }),
            _child: child,
        })
    }

    /// Take the reader half. Can only be called once.
    pub fn take_reader(&mut self) -> PtyReader {
        self.reader.take().expect("Reader already taken")
    }

    /// Write input bytes to the PTY (keyboard input).
    pub fn write(&mut self, data: &[u8]) -> std::io::Result<()> {
        self.writer.write_all(data)?;
        self.writer.flush()
    }
}
