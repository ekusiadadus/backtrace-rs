use std::fmt;
use std::mem;
use std::os::raw::c_void;
use std::path::{Path, PathBuf};

use {trace, resolve, Frame, Symbol, SymbolName};

/// Representation of an owned and self-contained backtrace.
///
/// This structure can be used to capture a backtrace at various points in a
/// program and later used to inspect what the backtrace was at that time.
pub struct Backtrace {
    frames: Box<[BacktraceFrame]>,
}

/// Captured version of a frame in a backtrace.
///
/// This type is returned as a list from `Backtrace::frames` and represents one
/// stack frame in a captured backtrace.
pub struct BacktraceFrame {
    ip: usize,
    symbol_address: usize,
    symbols: Box<[BacktraceSymbol]>,
}

/// Captured version of a symbol in a backtrace.
///
/// This type is returned as a list from `BacktraceFrame::symbols` and
/// represents the metadata for a symbol in a backtrace.
pub struct BacktraceSymbol {
    name: Option<Box<[u8]>>,
    addr: Option<usize>,
    filename: Option<PathBuf>,
    lineno: Option<u32>,
}

impl Backtrace {
    /// Captures a backtrace at the callsite of this function, returning an
    /// owned representation.
    ///
    /// This function is useful for representing a backtrace as an object in
    /// Rust. This returned value can be sent across threads and printed
    /// elsewhere, and thie purpose of this value is to be entirely self
    /// contained.
    ///
    /// # Examples
    ///
    /// ```
    /// use backtrace::Backtrace;
    ///
    /// let current_backtrace = Backtrace::new();
    /// ```
    pub fn new() -> Backtrace {
        let mut frames = Vec::new();
        trace(|frame| {
            let mut symbols = Vec::new();
            resolve(frame.ip(), |symbol| {
                symbols.push(BacktraceSymbol {
                    name: symbol.name().map(|m| m.as_bytes().to_vec().into_boxed_slice()),
                    addr: symbol.addr().map(|a| a as usize),
                    filename: symbol.filename().map(|m| m.to_path_buf()),
                    lineno: symbol.lineno(),
                });
            });
            frames.push(BacktraceFrame {
                ip: frame.ip() as usize,
                symbol_address: frame.symbol_address() as usize,
                symbols: symbols.into_boxed_slice(),
            });
            true
        });

        Backtrace { frames: frames.into_boxed_slice() }
    }

    /// Returns the frames from when this backtrace was captured.
    ///
    /// The first entry of this slice is likely the function `Backtrace::new`,
    /// and the last frame is likely something about how this thread or the main
    /// function started.
    pub fn frames(&self) -> &[BacktraceFrame] {
        &self.frames
    }
}

impl Frame for BacktraceFrame {
    fn ip(&self) -> *mut c_void {
        self.ip as *mut c_void
    }

    fn symbol_address(&self) -> *mut c_void {
        self.symbol_address as *mut c_void
    }
}

impl BacktraceFrame {
    /// Returns the list of symbols that this frame corresponds to.
    ///
    /// Normally there is only one symbol per frame, but sometimes if a number
    /// of functions are inlined into one frame then multiple symbols will be
    /// returned. The first symbol listed is the "innermost function", whereas
    /// the last symbol is the outermost (last caller).
    pub fn symbols(&self) -> &[BacktraceSymbol] {
        &self.symbols
    }
}

impl Symbol for BacktraceSymbol {
    fn name(&self) -> Option<SymbolName> {
        self.name.as_ref().map(|s| SymbolName::new(s))
    }

    fn addr(&self) -> Option<*mut c_void> {
        self.addr.map(|s| s as *mut c_void)
    }

    fn filename(&self) -> Option<&Path> {
        self.filename.as_ref().map(|p| &**p)
    }

    fn lineno(&self) -> Option<u32> {
        self.lineno
    }
}

impl fmt::Debug for Backtrace {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let hex_width = mem::size_of::<usize>() * 2 + 2;

        for (i, frame) in self.frames().iter().enumerate() {
            let ip = frame.ip();
            try!(write!(f, "frame #{:<2} - {:#02$x}", i, ip as usize, hex_width));

            if frame.symbols().len() == 0 {
                try!(writeln!(f, " - <no info>"));
                continue
            }

            for (j, symbol) in frame.symbols().iter().enumerate() {
                if j != 0 {
                    for _ in 0..7 + 2 + 3 + hex_width {
                        try!(write!(f, " "));
                    }
                }

                if let Some(name) = symbol.name() {
                    try!(write!(f, " - {}", name));
                } else {
                    try!(write!(f, " - <unknown>"));
                }
                if let Some(file) = symbol.filename() {
                    if let Some(l) = symbol.lineno() {
                        try!(write!(f, "\n{:13}{:4$}@ {}:{}", "", "",
                                    file.display(), l, hex_width));
                    }
                }
                try!(writeln!(f, ""));
            }
        }

        Ok(())
    }
}
