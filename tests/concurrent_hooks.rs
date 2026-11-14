//! Separate aria2 completion-hook processes sharing one configuration and log.
mod common;

use common::Fixture;
use std::fs::{self, File};
use std::path::Path;
use std::process::{Child, Stdio};
use std::time::{Duration, Instant};

// Kill and reap children even if an assertion fails: a regression must not leave
// movers running against a fixture that is being deleted.
struct Hooks(Vec<Child>);

impl Drop for Hooks {
    fn drop(&mut self) {
        for child in &mut self.0 {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

fn run_batch(completed: Option<&Path>) {
    let fx = Fixture::new();
    run_batch_with(&fx, completed);
}

fn run_batch_with(fx: &Fixture, completed: Option<&Path>) {
    let completed = completed.unwrap_or(&fx.completed);
    // Exercise concurrent creation of a missing completed directory too.
    if completed.exists() {
        fs::remove_dir(completed).unwrap();
    }
    let sizes = [
        0, 1, 4095, 65537, 1_048_575, 1_048_576, 1_048_577, 4_194_321,
    ];
    let mut expected = Vec::new();
    let mut hooks = Hooks(Vec::new());
    for round in 0..3 {
        for (index, size) in sizes.into_iter().enumerate() {
            let name = format!("download {round}-{index} café.bin");
            let bytes: Vec<_> = (0..size)
                .map(|offset| ((offset + index * 17 + round * 31) % 251) as u8)
                .collect();
            // Mix ordinary files and multi-file downloads, using aria2's first
            // nested file argument for the latter.
            let tree = index % 2 == 1;
            let source = if tree {
                fx.file(&format!("{name}/nested/first.bin"), &bytes)
            } else {
                fx.file(&name, &bytes)
            };
            if tree {
                fx.file(&format!("{name}/second.txt"), b"second file");
                fs::create_dir(fx.download.join(&name).join("empty")).unwrap();
            }
            let log = fx.root.join(format!("child-{round}-{index}.log"));
            let output = File::create(&log).unwrap();
            let child = fx
                .command()
                .arg("--completed-base")
                .arg(completed)
                .args(["--log-level", "verbose"])
                .arg(format!("gid-{round}-{index}"))
                .arg(if tree { "2" } else { "1" })
                .arg(&source)
                .stdout(Stdio::from(output.try_clone().unwrap()))
                .stderr(Stdio::from(output))
                .spawn()
                .unwrap();
            hooks.0.push(child);
            expected.push((name, bytes, tree, log));
        }
    }
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        let mut finished = true;
        for (child, (name, _, _, log)) in hooks.0.iter_mut().zip(&expected) {
            match child.try_wait().unwrap() {
                Some(status) => assert!(
                    status.success(),
                    "{name}: {status}\n{}",
                    fs::read_to_string(log).unwrap()
                ),
                None => finished = false,
            }
        }
        if finished {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "completion hooks did not finish within 60 seconds"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    for (name, bytes, tree, _) in &expected {
        let destination = completed.join(name);
        let file = if *tree {
            destination.join("nested/first.bin")
        } else {
            destination.clone()
        };
        assert_eq!(fs::read(file).unwrap(), *bytes, "{name}");
        if *tree {
            assert_eq!(
                fs::read(destination.join("second.txt")).unwrap(),
                b"second file"
            );
            assert!(destination.join("empty").is_dir());
        }
        assert!(!fx.download.join(name).exists(), "source retained: {name}");
    }
    assert!(
        common::names(&fx.download).is_empty(),
        "sources or locks remain"
    );
    assert_eq!(
        common::names(completed).len(),
        expected.len(),
        "unexpected destinations or staging directories"
    );
    let log = fs::read_to_string(fx.root.join("aria_move.log")).unwrap();
    assert_eq!(
        log.matches("Move completed").count(),
        expected.len(),
        "{log}"
    );
    assert!(!log.contains("Move failed"), "{log}");
}

#[test]
fn concurrent_aria2_hooks_finish_different_names_sizes_and_trees() {
    run_batch(None);
}

#[test]
fn concurrent_aria2_hooks_finish_across_filesystems() {
    let Some(other) = common::other_filesystem() else {
        eprintln!("skipped: no second filesystem available");
        return;
    };
    run_batch(Some(&other.path().join("completed")));
}

#[test]
#[ignore = "requires ARIA_MOVE_TEST_SHARE pointing to explicitly authorized test storage"]
fn concurrent_hooks_on_real_share() {
    let root = std::env::var_os("ARIA_MOVE_TEST_SHARE")
        .expect("set ARIA_MOVE_TEST_SHARE to authorize real-storage tests");
    let root = Path::new(&root);
    assert!(
        root.is_dir(),
        "test share is unavailable: {}",
        root.display()
    );
    // All writes and automatic cleanup stay inside owned temporary subtrees.
    let share = Fixture::new_in(root);
    eprintln!("local -> share: {}", share.completed.display());
    run_batch(Some(&share.completed));
    let local = Fixture::new();
    eprintln!("share -> local: {}", share.download.display());
    run_batch_with(&share, Some(&local.completed));
    // The previous batch leaves completed files and log records: use a fresh
    // fixture for the same-share rename case.
    let same_share = Fixture::new_in(root);
    eprintln!("share -> share: {}", same_share.root.display());
    run_batch_with(&same_share, None);
    share.close();
    same_share.close();
    eprintln!("PASS: 72 hook processes; all bytes verified; no sources, locks or staging remain");
}

#[test]
fn a_blocked_hook_does_not_prevent_other_downloads_finishing() {
    let fx = Fixture::new();
    let blocked = fx.file("blocked.bin", b"blocked payload");
    let held = aria_move::fs_ops::acquire_source_lock(&blocked).unwrap();
    let output = File::create(fx.root.join("blocked-output.log")).unwrap();
    let mut hooks = Hooks(vec![
        fx.command()
            .args(["--log-level", "verbose", "blocked-gid", "1"])
            .arg(&blocked)
            .stdout(Stdio::from(output.try_clone().unwrap()))
            .stderr(Stdio::from(output))
            .spawn()
            .unwrap(),
    ]);
    // Observe the child's lock attempt, rather than assuming startup timing.
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        let log = fs::read_to_string(fx.root.join("aria_move.log")).unwrap_or_default();
        if log.contains("Waiting for source lock") {
            break;
        }
        assert!(
            hooks.0[0].try_wait().unwrap().is_none(),
            "blocked hook exited early"
        );
        assert!(
            Instant::now() < deadline,
            "hook did not reach lock acquisition"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    // Each sibling must finish while the first hook is still blocked.
    for index in 0..8 {
        let name = format!("sibling-{index}.bin");
        let source = fx.file(&name, &[index; 4096]);
        let output = File::create(fx.root.join(format!("sibling-{index}.log"))).unwrap();
        hooks.0.push(
            fx.command()
                .args(["sibling-gid", "1"])
                .arg(&source)
                .stdout(Stdio::from(output.try_clone().unwrap()))
                .stderr(Stdio::from(output))
                .spawn()
                .unwrap(),
        );
    }
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        let statuses: Vec<_> = hooks.0[1..]
            .iter_mut()
            .map(|child| child.try_wait().unwrap())
            .collect();
        assert!(statuses.iter().flatten().all(|status| status.success()));
        if statuses.iter().all(Option::is_some) {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "siblings stalled behind a different source lock"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(hooks.0[0].try_wait().unwrap().is_none());
    assert!(blocked.exists());
    for index in 0..8 {
        assert_eq!(
            fs::read(fx.completed.join(format!("sibling-{index}.bin"))).unwrap(),
            [index; 4096]
        );
    }
    drop(held);
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        if let Some(status) = hooks.0[0].try_wait().unwrap() {
            assert!(status.success());
            break;
        }
        assert!(
            Instant::now() < deadline,
            "blocked hook did not resume after lock release"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(
        fs::read(fx.completed.join("blocked.bin")).unwrap(),
        b"blocked payload"
    );
    assert!(common::names(&fx.download).is_empty());
    assert_eq!(fx.completed_names().len(), 9);
}
