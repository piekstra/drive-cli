//! The environment every black-box test runs gdrv in: its own home and
//! config directories, an empty keychain service, and the vault's key in a
//! file (`GDRV_TEST_KEY_FILE`, a debug-build hook), so no test reads the real
//! keychain, vault or profiles. With a fake Drive server, requests go to it on
//! 127.0.0.1 and nowhere else.

#![allow(dead_code)]

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Output, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};

use assert_cmd::Command;
use pk_cli_drive::google::fake::{FakeDrive, CLIENT_JSON};
use serde_json::Value;

pub const PROFILE: &str = "home";

static SEQ: AtomicUsize = AtomicUsize::new(0);

pub struct Env {
    pub root: PathBuf,
    pub fake: Option<FakeDrive>,
    _dir: tempfile::TempDir,
}

impl Env {
    /// No Drive server: every request would go nowhere.
    pub fn offline() -> Env {
        let dir = tempfile::Builder::new()
            .prefix(&format!(
                "gdrv-test-{}-",
                SEQ.fetch_add(1, Ordering::SeqCst)
            ))
            .tempdir()
            .unwrap();
        Env {
            root: dir.path().to_path_buf(),
            fake: None,
            _dir: dir,
        }
    }

    /// A fake Drive server, not yet signed in.
    pub fn with_fake() -> Env {
        let mut e = Env::offline();
        e.fake = Some(FakeDrive::start());
        e
    }

    /// A fake Drive server, profile `home` signed in.
    pub fn signed_in() -> Env {
        let e = Env::with_fake();
        e.login(&[]);
        e
    }

    pub fn fake(&self) -> &FakeDrive {
        self.fake.as_ref().expect("a fake Drive server")
    }

    pub fn path(&self, rel: &str) -> PathBuf {
        self.root.join(rel)
    }

    fn envs(&self) -> Vec<(String, String)> {
        let mut v = vec![
            ("HOME".into(), self.root.display().to_string()),
            (
                "XDG_CONFIG_HOME".into(),
                self.root.join("config").display().to_string(),
            ),
            (
                "XDG_CACHE_HOME".into(),
                self.root.join("cache").display().to_string(),
            ),
            (
                "GDRV_KEYCHAIN_SERVICE".into(),
                "piekstra.gdrv.hermetic-tests".into(),
            ),
            (
                "GDRV_TEST_KEY_FILE".into(),
                self.root.join("keys.json").display().to_string(),
            ),
        ];
        if let Some(f) = &self.fake {
            v.push(("GDRV_DRIVE_TEST_ENDPOINT".into(), f.base_url()));
        }
        v
    }

    pub fn cmd(&self) -> Command {
        let mut c = Command::cargo_bin("gdrv").unwrap();
        c.env_clear()
            .env("PATH", std::env::var("PATH").unwrap_or_default())
            .envs(self.envs());
        c
    }

    pub fn run(&self, args: &[&str]) -> Output {
        self.cmd().args(args).output().unwrap()
    }

    pub fn run_stdin(&self, args: &[&str], stdin: &[u8]) -> Output {
        self.cmd().args(args).write_stdin(stdin).output().unwrap()
    }

    /// A `--json` run that must succeed: its one document.
    pub fn ok(&self, args: &[&str]) -> Value {
        let mut a = vec!["--json"];
        a.extend_from_slice(args);
        let out = self.run(&a);
        assert!(
            out.status.success(),
            "gdrv {args:?} exited {:?}\nstdout: {}\nstderr: {}",
            out.status.code(),
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        one_document(&out)
    }

    /// A `--json` run that must fail with `code`: its error object.
    pub fn fails(&self, code: i32, args: &[&str]) -> Value {
        let mut a = vec!["--json"];
        a.extend_from_slice(args);
        let out = self.run(&a);
        assert_eq!(
            out.status.code(),
            Some(code),
            "gdrv {args:?}\nstdout: {}\nstderr: {}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        one_document(&out)["error"].clone()
    }

    /// `gdrv login --profile home`, the client JSON on stdin, the fake's
    /// sign-in page followed to the loopback listener the login waits on.
    pub fn login(&self, extra: &[&str]) -> Value {
        let mut args = vec![
            "--json",
            "login",
            "--profile",
            PROFILE,
            "--client-stdin",
            "--no-browser",
            "--wait",
            "1",
        ];
        args.extend_from_slice(extra);
        let mut child = std::process::Command::new(assert_cmd::cargo::cargo_bin("gdrv"))
            .env_clear()
            .env("PATH", std::env::var("PATH").unwrap_or_default())
            .envs(self.envs())
            .args(&args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(CLIENT_JSON.as_bytes())
            .unwrap();
        let mut stderr = BufReader::new(child.stderr.take().unwrap());
        let mut seen = String::new();
        let url = loop {
            let mut line = String::new();
            if stderr.read_line(&mut line).unwrap() == 0 {
                panic!("login printed no sign-in address: {seen}");
            }
            seen.push_str(&line);
            if line.starts_with("http://127.0.0.1") {
                break line.trim().to_string();
            }
        };
        let rest = std::thread::spawn(move || {
            let mut s = String::new();
            let _ = std::io::Read::read_to_string(&mut stderr, &mut s);
            s
        });
        let _ = reqwest::blocking::get(&url);
        let out = child.wait_with_output().unwrap();
        let err = rest.join().unwrap();
        assert!(
            out.status.success(),
            "login exited {:?}: {}{seen}{err}",
            out.status.code(),
            String::from_utf8_lossy(&out.stdout)
        );
        one_document(&out)
    }

    pub fn write(&self, rel: &str, bytes: &[u8]) -> PathBuf {
        let p = self.path(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(&p, bytes).unwrap();
        p
    }
}

pub fn one_document(out: &Output) -> Value {
    serde_json::from_slice(&out.stdout).unwrap_or_else(|e| {
        panic!(
            "stdout is not one JSON document ({e}): {}",
            String::from_utf8_lossy(&out.stdout)
        )
    })
}

pub fn s(p: &Path) -> &str {
    p.to_str().unwrap()
}

pub fn md5_hex(bytes: &[u8]) -> String {
    let path = std::env::temp_dir().join(format!(
        "gdrv-md5-{}-{}",
        std::process::id(),
        SEQ.fetch_add(1, Ordering::SeqCst)
    ));
    std::fs::write(&path, bytes).unwrap();
    let h = pk_cli_drive::google::md5_file(&path).unwrap();
    let _ = std::fs::remove_file(&path);
    h
}
