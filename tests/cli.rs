//! End-to-end behaviour of the `aria_move` binary.
mod common;

use common::{Fixture, assert_failure, assert_success, snapshot, stdout};
use std::fs;
use std::path::PathBuf;

#[test]
fn aria2_hook_arguments_move_files_and_directories() {
    let fx = Fixture::new();
    let file = fx.file("ok.bin", b"content");
    assert_success(&fx.run(&[&"7b3f1234", &"1", &file]));
    assert_eq!(fs::read(fx.completed.join("ok.bin")).unwrap(), b"content");
    assert!(!file.exists());

    fx.file("dir space/nested.txt", b"nested");
    let dir = fx.download.join("dir space");
    assert_success(&fx.run(&[&"GID", &"1", &dir]));
    assert_eq!(
        fs::read(fx.completed.join("dir space/nested.txt")).unwrap(),
        b"nested"
    );
    assert!(!dir.exists());
}

#[test]
fn single_argument_forms_resolve_against_the_download_base() {
    // (argument, entry created under incoming/, path expected under completed/)
    let cases: [(&str, &str, &str); 4] = [
        ("bare.txt", "bare.txt", "bare.txt"),
        ("New folder", "New folder/file.txt", "New folder/file.txt"),
        ("Trailing/", "Trailing/t.txt", "Trailing/t.txt"),
        ("--source-path=flagged.bin", "flagged.bin", "flagged.bin"),
    ];
    for (argument, created, expected) in cases {
        let fx = Fixture::new();
        fx.file(created, b"data");
        assert_success(&fx.run(&[&argument]));
        assert_eq!(
            fs::read(fx.completed.join(expected)).unwrap(),
            b"data",
            "{argument}"
        );
        assert!(common::names(&fx.download).is_empty(), "{argument}");
    }
}

#[test]
fn a_nested_file_moves_its_whole_download_folder() {
    let fx = Fixture::new();
    fx.file("rootname/a.txt", b"A");
    let nested = fx.file("rootname/sub/b.txt", b"B");
    assert_success(&fx.run(&[&"TASK", &"2", &nested]));
    assert_eq!(fs::read(fx.completed.join("rootname/a.txt")).unwrap(), b"A");
    assert_eq!(
        fs::read(fx.completed.join("rootname/sub/b.txt")).unwrap(),
        b"B"
    );
    assert!(!fx.download.join("rootname").exists());
}

#[test]
fn quoted_paths_from_shell_wrappers_are_accepted() {
    let quoted: [fn(&PathBuf) -> String; 2] = [
        |p| format!("\"{}\"", p.display()),
        |p| format!("'{}\\'", p.display()), // PowerShell single quotes with a trailing backslash
    ];
    for quote in quoted {
        let fx = Fixture::new();
        fx.file("New folder/file.txt", b"data");
        let argument = quote(&fx.download.join("New folder"));
        assert_success(&fx.run(&[&"TASKID", &"1", &argument]));
        assert!(
            fx.completed.join("New folder/file.txt").exists(),
            "{argument}"
        );
    }
}

#[test]
fn an_existing_destination_is_kept_and_a_new_name_used() {
    let fx = Fixture::new();
    fs::write(fx.completed.join("file.iso"), b"older").unwrap();
    let file = fx.file("file.iso", b"newer");
    assert_success(&fx.run(&[&file]));
    assert_eq!(fs::read(fx.completed.join("file.iso")).unwrap(), b"older");
    let names = fx.completed_names();
    assert_eq!(names.len(), 2, "{names:?}");
    let renamed = names.iter().find(|n| *n != "file.iso").unwrap();
    assert!(
        renamed.starts_with("file-") && renamed.ends_with(".iso"),
        "{renamed}"
    );
    assert_eq!(fs::read(fx.completed.join(renamed)).unwrap(), b"newer");
}

#[test]
fn failures_exit_nonzero_with_a_reason_and_move_nothing() {
    let fx = Fixture::new();
    let untouched = fx.file("untouched.bin", b"data");
    let missing = fx.download.join("nope.bin");
    let cases: [(Vec<std::ffi::OsString>, &str); 4] = [
        (
            vec!["TASKID".into(), "1".into(), missing.into()],
            "Source path not found",
        ),
        (
            vec!["TASKID_ONLY".into(), "1".into()],
            "No source path provided",
        ),
        (
            vec!["TASKID".into(), "not-a-number".into(), "x".into()],
            "invalid value",
        ),
        (
            vec!["A".into(), "1".into(), "x".into(), "EXTRA".into()],
            "unexpected argument",
        ),
    ];
    for (args, message) in cases {
        let output = fx.command().args(&args).output().unwrap();
        assert_failure(&output, message);
        assert!(fx.completed_names().is_empty(), "{args:?}");
    }
    assert!(untouched.exists());
}

#[test]
fn incomplete_downloads_are_refused() {
    let fx = Fixture::new();
    let partial = fx.file("movie.mkv.part", b"partial");
    assert_failure(&fx.run(&[&partial]), "unfinished download");
    assert!(partial.exists());
}

#[test]
fn print_config_reports_the_selected_path() {
    let fx = Fixture::new();
    let cases = [
        (vec![], fx.config.clone()),
        (
            vec!["--config".into(), fx.root.join("other.xml")],
            fx.root.join("other.xml"),
        ),
        // A directory means its config.xml.
        (
            vec!["--config".into(), fx.download.clone()],
            fx.download.join("config.xml"),
        ),
    ];
    for (args, expected) in cases {
        let output = fx
            .command()
            .args(&args)
            .arg("--print-config")
            .output()
            .unwrap();
        assert_success(&output);
        assert!(
            stdout(&output).contains(&expected.display().to_string()),
            "{args:?}: {}",
            stdout(&output)
        );
    }
    // Relative ARIA_MOVE_CONFIG values resolve against the working directory.
    let output = fx
        .command()
        .env("ARIA_MOVE_CONFIG", "rel.xml")
        .arg("--print-config")
        .output()
        .unwrap();
    assert!(stdout(&output).contains(&fx.root.join("rel.xml").display().to_string()));
}

#[test]
fn config_flag_wins_over_the_environment() {
    let fx = Fixture::new();
    let other = Fixture::new();
    let source = other.file("dummy.bin", b"data");
    let output = fx.run(&[&"--config", &other.config, &"--dry-run", &source]);
    assert_success(&output);
    let text = stdout(&output);
    assert!(
        text.contains(&other.completed.display().to_string()),
        "{text}"
    );
    assert!(
        !text.contains(&fx.completed.display().to_string()),
        "{text}"
    );
}

#[test]
fn first_run_writes_a_template_that_must_be_edited() {
    let fx = Fixture::new();
    let config = fx.root.join("fresh").join("config.xml");
    let file = fx.file("file", b"x");
    let output = fx
        .command()
        .env("ARIA_MOVE_CONFIG", &config)
        .arg(&file)
        .output()
        .unwrap();
    assert_success(&output);
    assert!(stdout(&output).contains("template config was written"));
    assert!(file.exists(), "the first run never moves anything");
    let template = fs::read_to_string(&config).unwrap();
    assert!(template.contains("/path/to/incoming"));
    let output = fx
        .command()
        .env("ARIA_MOVE_CONFIG", &config)
        .arg(&file)
        .output()
        .unwrap();
    assert_failure(&output, "template placeholder");
    assert_eq!(
        fs::read_to_string(&config).unwrap(),
        template,
        "never rewritten"
    );
}

#[test]
fn bases_can_come_from_flags_alone() {
    let fx = Fixture::new();
    fs::write(&fx.config, "<config/>").unwrap();
    let file = fx.file("f.bin", b"x");
    assert_failure(&fx.run(&[&file]), "download_base is not set");
    let output = fx.run(&[
        &"--download-base",
        &fx.download,
        &"--completed-base",
        &fx.completed,
        &file,
    ]);
    assert_success(&output);
    assert!(fx.completed.join("f.bin").exists());
}

#[test]
fn dry_runs_write_nothing_at_all() {
    let fx = Fixture::new();
    fx.file("movie/a", b"source a");
    fs::create_dir_all(fx.completed.join("movie")).unwrap();
    fs::write(fx.completed.join("movie/unrelated"), b"do not touch").unwrap();
    fs::write(
        fx.completed.join(".aria_move.stage.stale"),
        b"not ours to clean",
    )
    .unwrap();
    let before = snapshot(&fx.root);
    let output = fx.run(&[&"--dry-run", &fx.download.join("movie")]);
    assert_success(&output);
    assert!(stdout(&output).contains("Dry-run: would move"));
    // A missing config is an error rather than a template in a dry run.
    let output = fx
        .command()
        .env("ARIA_MOVE_CONFIG", fx.root.join("missing/config.xml"))
        .arg("--dry-run")
        .output()
        .unwrap();
    assert_failure(&output, "Config missing");
    assert_eq!(
        snapshot(&fx.root),
        before,
        "no move, log, template or directory"
    );
}

#[test]
fn json_logs_are_one_object_per_line() {
    let fx = Fixture::new();
    let file = fx.file("file", b"keep");
    let output = fx.run(&[&"--json", &"--dry-run", &file]);
    assert_success(&output);
    let text = stdout(&output);
    assert!(text.contains("Move completed"), "{text}");
    for line in text.lines() {
        let value: serde_json::Value = serde_json::from_str(line).unwrap();
        assert!(value.get("level").is_some(), "{line}");
    }
}

#[test]
fn logs_default_beside_the_config_or_to_an_explicit_file() {
    let fx = Fixture::new();
    let file = fx.file("one", b"1");
    assert_success(&fx.run(&[&file]));
    let log = fs::read_to_string(fx.root.join("aria_move.log")).unwrap();
    assert!(log.contains("Move completed"), "{log}");

    let explicit = fx.root.join("logs").join("custom.log");
    let escaped = quick_xml::escape::escape(explicit.to_str().unwrap()).into_owned();
    fx.write_config(&format!(
        "<log_file>{escaped}</log_file><log_level>debug</log_level>"
    ));
    let file = fx.file("two", b"2");
    assert_success(&fx.run(&[&file]));
    let log = fs::read_to_string(&explicit).unwrap();
    assert!(
        log.contains("Move completed") && log.contains("two"),
        "{log}"
    );
}

#[test]
fn concurrent_processes_move_separate_downloads() {
    let fx = Fixture::new();
    for batch in ["batch_a", "batch_b"] {
        for i in 0..100 {
            fx.file(
                &format!("{batch}/{i:03}.dat"),
                format!("{batch}:{i}").as_bytes(),
            );
        }
    }
    let children: Vec<_> = ["batch_a", "batch_b"]
        .map(|batch| fx.command().arg(fx.download.join(batch)).spawn().unwrap())
        .into_iter()
        .collect();
    for child in children {
        assert!(child.wait_with_output().unwrap().status.success());
    }
    for batch in ["batch_a", "batch_b"] {
        let dest = fx.completed.join(batch);
        assert_eq!(common::names(&dest).len(), 100);
        assert_eq!(
            fs::read_to_string(dest.join("050.dat")).unwrap(),
            format!("{batch}:50")
        );
    }
    assert!(common::names(&fx.download).is_empty());
}

#[cfg(unix)]
mod unix {
    use super::common::{self, Fixture, assert_failure, assert_success};
    use std::fs;
    use std::os::unix::fs::{PermissionsExt, symlink};
    use std::process::Command;

    #[test]
    fn symlinks_and_special_files_are_refused() {
        let fx = Fixture::new();
        fs::create_dir(fx.root.join("real_dir")).unwrap();
        symlink(fx.root.join("real_dir"), fx.download.join("dir_link")).unwrap();
        fs::write(fx.root.join("real.bin"), b"x").unwrap();
        symlink(fx.root.join("real.bin"), fx.download.join("file_link")).unwrap();
        let fifo = fx.download.join("pipe");
        assert!(
            Command::new("mkfifo")
                .arg(&fifo)
                .status()
                .unwrap()
                .success()
        );
        for argument in ["dir_link", "dir_link/", "file_link"] {
            assert_failure(&fx.run(&[&argument]), "symlink");
        }
        assert_failure(&fx.run(&[&fifo]), "not a regular file");
        assert!(fx.completed_names().is_empty());
    }

    #[test]
    fn unix_quoting_and_byte_names() {
        use std::os::unix::ffi::OsStringExt;
        let fx = Fixture::new();
        fx.file("New folder/x.txt", b"x");
        let quoted = format!("'{}'/", fx.download.join("New folder").display());
        assert_success(&fx.run(&[&"TASK", &"1", &quoted]));
        assert!(fx.completed.join("New folder/x.txt").exists());

        // A literal quoted name is preferred over its unquoted twin.
        fx.file("'literal'", b"quoted");
        fx.file("literal", b"unquoted");
        assert_success(&fx.run(&[&"'literal'"]));
        assert_eq!(fs::read(fx.completed.join("'literal'")).unwrap(), b"quoted");
        assert_eq!(fs::read(fx.download.join("literal")).unwrap(), b"unquoted");

        let name = std::ffi::OsString::from_vec(b"data-\xff".to_vec());
        let source = fx.download.join(&name);
        fs::write(&source, b"bytes").unwrap();
        assert_success(
            &fx.command()
                .arg("--source-path")
                .arg(&source)
                .output()
                .unwrap(),
        );
        assert_eq!(fs::read(fx.completed.join(name)).unwrap(), b"bytes");
    }

    #[test]
    fn lock_failures_suggest_and_honour_disable_locks() {
        if common::is_root() {
            return;
        }
        let fx = Fixture::new();
        let file = fx.file("data", b"keep");
        // Without read permission the directory cannot be opened to lock it.
        fs::set_permissions(&fx.download, fs::Permissions::from_mode(0o333)).unwrap();
        let refused = fx.run(&[&file]);
        let environment = fx
            .command()
            .env("ARIA_MOVE_DISABLE_LOCKS", "1")
            .arg(&file)
            .output()
            .unwrap();
        fs::set_permissions(&fx.download, fs::Permissions::from_mode(0o700)).unwrap();
        assert_failure(&refused, "--disable-locks");
        assert_success(&environment);
        assert_eq!(fs::read(fx.completed.join("data")).unwrap(), b"keep");
    }

    #[test]
    fn sigterm_interrupts_a_wait_for_a_lock() {
        use std::io::{BufRead, BufReader};
        use std::process::Stdio;
        use std::time::{Duration, Instant};
        let fx = Fixture::new();
        let source = fx.file("file", b"keep");
        let held = aria_move::fs_ops::acquire_dir_lock(&fx.download).unwrap();
        let mut child = fx
            .command()
            .args(["--debug", "--json"])
            .arg(&source)
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let stdout = child.stdout.take().unwrap();
        let (sender, receiver) = std::sync::mpsc::channel();
        let reader = std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                if line.unwrap().contains("Waiting for directory lock") {
                    let _ = sender.send(());
                }
            }
        });
        let waiting = receiver.recv_timeout(Duration::from_secs(10)).is_ok();
        if waiting {
            // SAFETY: signals only our own live child, whose handler catches SIGTERM.
            assert_eq!(unsafe { libc::kill(child.id() as i32, libc::SIGTERM) }, 0);
        }
        let deadline = Instant::now() + Duration::from_secs(10);
        let status = loop {
            if let Some(status) = child.try_wait().unwrap() {
                break status;
            }
            if Instant::now() >= deadline {
                child.kill().unwrap();
                break child.wait().unwrap();
            }
            std::thread::sleep(Duration::from_millis(10));
        };
        reader.join().unwrap();
        drop(held);
        assert!(waiting, "the child never reached the lock wait");
        assert_eq!(status.code(), Some(1), "SIGTERM must be handled, not fatal");
        assert_eq!(fs::read(source).unwrap(), b"keep");
        assert!(fx.completed_names().is_empty());
    }
}
