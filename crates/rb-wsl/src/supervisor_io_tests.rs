use tokio::io::AsyncWriteExt;

use super::*;

#[tokio::test]
async fn an_endless_stderr_line_is_skipped_not_kept_in_memory() {
    let (mut writer, reader) = tokio::io::duplex(8192);
    let kept = KeptStderr::default();
    let task = tokio::spawn(read_stderr(reader, kept.clone()));
    let long = vec![b'x'; MAX_LINE + 10];
    writer.write_all(&long).await.expect("writes");
    writer
        .write_all(b"\nexecvpe failed\n")
        .await
        .expect("writes");
    drop(writer);
    task.await.expect("the reader ends");
    assert_eq!(kept.text(), "execvpe failed");
    let all = kept.0.lock().expect("not poisoned").clone();
    assert!(!all.contains("xxxx"), "the long line was kept");
}
