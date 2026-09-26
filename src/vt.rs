//! Outbound terminal protocols — OSC notifications, attention, prompt
//! marks, window title — as typed byte builders.
//!
//! Lifted from mado's `vt.rs` (the terminal that also PARSES these), so
//! every fleet program that writes to a terminal — mado itself, arnes and
//! every egaku-term TUI — emits the same bytes from one place. The envelope
//! grammar (introducer, `;` separators, terminator) lives in [`osc`] once;
//! call sites declare the numeric code and typed parameters, never escape
//! bytes. Dependency-free and renderer-free, like the rest of egaku.

use std::io::Write as _;

/// How an OSC string is terminated: `BEL` (0x07) or `ST` (`ESC \`).
/// Both are valid per ECMA-48; individual protocols pick one (iTerm2 OSC 9
/// and OSC 1337 use BEL; the kitty OSC 99 protocol uses ST).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OscTerminator {
    /// `BEL` — 0x07.
    Bel,
    /// `ST` — `ESC \`.
    St,
}

/// An OSC sequence: `ESC ]` · numeric code · `;`-joined string params ·
/// terminator.
#[must_use]
pub fn osc(code: u16, params: &[&str], terminator: OscTerminator) -> Vec<u8> {
    let total: usize = params.iter().map(|p| p.len() + 1).sum();
    let mut out = Vec::with_capacity(6 + total);
    out.extend_from_slice(b"\x1b]");
    let _ = write!(out, "{code}");
    for p in params {
        out.push(b';');
        out.extend_from_slice(p.as_bytes());
    }
    match terminator {
        OscTerminator::Bel => out.push(0x07),
        OscTerminator::St => out.extend_from_slice(b"\x1b\\"),
    }
    out
}

/// The terminal bell, `BEL` (0x07) — the lowest common attention signal.
pub const BELL: &[u8] = b"\x07";

/// OSC 2: set the window title. `ESC ] 2 ; <title> BEL`.
#[must_use]
pub fn window_title(title: &str) -> Vec<u8> {
    osc(2, &[title], OscTerminator::Bel)
}

/// OSC 9 (iTerm2) simple notification: `ESC ] 9 ; <body> BEL`.
#[must_use]
pub fn osc9_notify(body: &str) -> Vec<u8> {
    osc(9, &[body], OscTerminator::Bel)
}

/// OSC 777 (urxvt/foot) notification:
/// `ESC ] 777 ; notify ; <title> ; <body> BEL`.
#[must_use]
pub fn osc777_notify(title: &str, body: &str) -> Vec<u8> {
    osc(777, &["notify", title, body], OscTerminator::Bel)
}

/// Which field an OSC 99 payload chunk carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Osc99Part {
    /// The notification title.
    Title,
    /// The notification body.
    Body,
}

impl Osc99Part {
    fn as_str(self) -> &'static str {
        match self {
            Osc99Part::Title => "title",
            Osc99Part::Body => "body",
        }
    }
}

/// One chunk of a kitty OSC 99 notification:
/// `ESC ] 99 ; i=<id>:d=<0|1>:u=<urgency>:p=<part> ; <payload> ST`.
///
/// `done` marks the final chunk (the receiver renders on `d=1`);
/// `urgency` is 0 (low) / 1 (normal) / 2 (critical).
#[must_use]
pub fn osc99_notify(id: &str, done: bool, urgency: u8, part: Osc99Part, payload: &str) -> Vec<u8> {
    use std::fmt::Write as _;
    let mut meta = String::with_capacity(id.len() + 20);
    let _ = write!(meta, "i={id}:d={}:u={urgency}:p={}", u8::from(done), part.as_str());
    osc(99, &[&meta, payload], OscTerminator::St)
}

/// OSC 1337 (iTerm2) `RequestAttention`: `ESC ] 1337 ; RequestAttention=<0|1> BEL`.
#[must_use]
pub fn osc1337_request_attention(on: bool) -> Vec<u8> {
    let param = if on { "RequestAttention=1" } else { "RequestAttention=0" };
    osc(1337, &[param], OscTerminator::Bel)
}

/// A shell-integration semantic prompt mark (OSC 133 / `FinalTerm`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Osc133Mark {
    /// `A` — prompt start (fresh line + start of prompt).
    PromptStart,
    /// `B` — end of prompt, start of command input.
    CommandStart,
    /// `C` — command output begins (the command is now executing).
    CommandOutput,
    /// `D` — command finished; `Some(code)` carries the exit status.
    CommandEnd(Option<i32>),
}

/// Emit one OSC 133 mark: `ESC ] 133 ; <letter>[;<exit>] ST`.
#[must_use]
pub fn osc133(mark: Osc133Mark) -> Vec<u8> {
    match mark {
        Osc133Mark::PromptStart => osc(133, &["A"], OscTerminator::St),
        Osc133Mark::CommandStart => osc(133, &["B"], OscTerminator::St),
        Osc133Mark::CommandOutput => osc(133, &["C"], OscTerminator::St),
        Osc133Mark::CommandEnd(None) => osc(133, &["D"], OscTerminator::St),
        Osc133Mark::CommandEnd(Some(code)) => {
            let s = code.to_string();
            osc(133, &["D", &s], OscTerminator::St)
        }
    }
}

/// A desktop notification in every dialect at once — OSC 9 (iTerm2,
/// WezTerm, Ghostty, mado), OSC 777 (foot, urxvt) and OSC 99 (kitty).
/// Terminals ignore the dialects they do not speak, so one call reaches
/// whichever terminal the operator is in.
#[must_use]
pub fn notify_all(title: &str, body: &str) -> Vec<u8> {
    let mut out = osc9_notify(&format!("{title}: {body}"));
    out.extend(osc777_notify(title, body));
    out.extend(osc99_notify("n", false, 1, Osc99Part::Title, title));
    out.extend(osc99_notify("n", true, 1, Osc99Part::Body, body));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    // Byte-exact tests carried over from mado's vt.rs with the builders.

    #[test]
    fn osc_envelope_bel_and_st() {
        assert_eq!(osc(9, &["hi"], OscTerminator::Bel), b"\x1b]9;hi\x07");
        assert_eq!(osc(99, &["m", "p"], OscTerminator::St), b"\x1b]99;m;p\x1b\\");
        assert_eq!(osc(0, &[], OscTerminator::Bel), b"\x1b]0\x07");
    }

    #[test]
    fn osc9_and_777_builders_are_byte_exact() {
        assert_eq!(osc9_notify("Build done"), b"\x1b]9;Build done\x07");
        assert_eq!(
            osc777_notify("Mado", "All tests passed"),
            b"\x1b]777;notify;Mado;All tests passed\x07"
        );
    }

    #[test]
    fn osc99_builder_encodes_metadata_and_payload() {
        assert_eq!(
            osc99_notify("t1", false, 2, Osc99Part::Title, "Title"),
            b"\x1b]99;i=t1:d=0:u=2:p=title;Title\x1b\\"
        );
        assert_eq!(
            osc99_notify("t1", true, 1, Osc99Part::Body, "Body"),
            b"\x1b]99;i=t1:d=1:u=1:p=body;Body\x1b\\"
        );
    }

    #[test]
    fn osc1337_request_attention_builder() {
        assert_eq!(osc1337_request_attention(true), b"\x1b]1337;RequestAttention=1\x07");
        assert_eq!(osc1337_request_attention(false), b"\x1b]1337;RequestAttention=0\x07");
    }

    #[test]
    fn osc133_marks_build() {
        assert_eq!(osc133(Osc133Mark::PromptStart), b"\x1b]133;A\x1b\\");
        assert_eq!(osc133(Osc133Mark::CommandStart), b"\x1b]133;B\x1b\\");
        assert_eq!(osc133(Osc133Mark::CommandOutput), b"\x1b]133;C\x1b\\");
        assert_eq!(osc133(Osc133Mark::CommandEnd(None)), b"\x1b]133;D\x1b\\");
        assert_eq!(osc133(Osc133Mark::CommandEnd(Some(0))), b"\x1b]133;D;0\x1b\\");
        assert_eq!(osc133(Osc133Mark::CommandEnd(Some(130))), b"\x1b]133;D;130\x1b\\");
    }

    #[test]
    fn window_title_and_notify_all() {
        assert_eq!(window_title("arnes"), b"\x1b]2;arnes\x07");
        let all = notify_all("arnes", "done");
        assert!(all.starts_with(b"\x1b]9;arnes: done\x07"));
        assert!(all.windows(4).any(|w| w == b"]777"));
        assert!(all.windows(3).any(|w| w == b"]99"));
    }
}
