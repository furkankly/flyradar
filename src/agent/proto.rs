use std::sync::Arc;

use tokio::io::{
    AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, {self},
};
use tokio::sync::Mutex;

pub async fn read<T: AsyncRead + Unpin>(reader: Arc<Mutex<T>>) -> io::Result<Vec<u8>> {
    let mut reader = reader.lock().await;
    // Read length prefix (2 bytes)
    let mut len_bytes = [0u8; 2];
    reader.read_exact(&mut len_bytes).await?;
    let len = u16::from_le_bytes(len_bytes) as usize;

    // Allocate buffer for data
    let mut data = vec![0u8; len];
    reader.read_exact(&mut data).await?;
    Ok(data)
}

pub async fn write<T: AsyncWrite + Unpin>(
    writer: Arc<Mutex<T>>,
    verb: &str,
    args: &[&str],
) -> io::Result<()> {
    let mut writer = writer.lock().await;
    // Calculate total size: verb + spaces + args
    let size = verb.len() + args.iter().map(|arg| arg.len() + 1).sum::<usize>();

    // Write length as little-endian u16
    writer.write_all(&(size as u16).to_le_bytes()).await?;

    // Write verb
    writer.write_all(verb.as_bytes()).await?;

    // Write args with spaces
    for arg in args {
        writer.write_all(b" ").await?;
        writer.write_all(arg.as_bytes()).await?;
    }

    // Ensure all data is sent
    writer.flush().await?;

    Ok(())
}

// #[cfg(test)]
// mod tests {
//     use super::*;
//     use tokio::io::BufWriter;
//
//     #[tokio::test]
//     async fn test_write_and_read() {
//         let mut buffer = Vec::new();
//         let mut writer = BufWriter::new(&mut buffer);
//         write(&mut writer, "test", &["arg1", "arg2"]).await.unwrap();
//         writer.flush().await.unwrap();
//
//         let mut cursor = tokio::io::BufReader::new(buffer.as_slice());
//         let result = read(&mut cursor).await.unwrap();
//         assert_eq!(String::from_utf8(result).unwrap(), "test arg1 arg2");
//     }
//
//     #[tokio::test]
//     async fn test_read_empty() {
//         let buffer = vec![0, 0]; // Length of 0
//         let mut cursor = tokio::io::BufReader::new(buffer.as_slice());
//         let result = read(&mut cursor).await.unwrap();
//         assert!(result.is_empty());
//     }
//
//     #[tokio::test]
//     async fn test_size_calculation() {
//         let mut buffer = Vec::new();
//         let mut writer = BufWriter::new(&mut buffer);
//         write(&mut writer, "test", &["arg1", "arg2"]).await.unwrap();
//         writer.flush().await.unwrap();
//         assert_eq!(&buffer[..2], &14u16.to_le_bytes()); // First 2 bytes should be 14
//     }
// }
