//! Async read and write helpers for decoding and encoding Tibia 1.03 network packets.
//!
//! Two extension traits are provided:
//! - [`ReadExt`] - blanket-implemented for any [`AsyncRead`] + [`Unpin`] type.
//! - [`WriteExt`] - blanket-implemented for any [`AsyncWrite`] + [`Unpin`] type.

use crate::{
    map::position::Position,
    network::packet::PacketOut,
    player::{Gender, OutfitColors},
};
use anyhow::Result;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

// Blanket impl so any AsyncRead + Unpin automatically gets ReadExt.
impl<R: AsyncRead + Unpin> ReadExt for R {}

/// Read helpers for decoding Tibia 1.03 network packets.
pub trait ReadExt: AsyncRead + Unpin + Sized {
    /// Reads one byte and splits it into two packed 4-bit values `(high, low)`.
    async fn read_u4(&mut self) -> Result<(u8, u8)> {
        let byte = self.read_u8().await?;
        Ok((byte / 16, byte % 16))
    }

    /// Reads a fixed-size, null-terminated string field of `max_size` bytes.
    ///
    /// Characters are read until a null terminator (`\0`) is encountered. The
    /// remaining bytes in the field are consumed and discarded so the stream
    /// stays aligned for the next read.
    async fn read_string(&mut self, buf: &mut String, max_size: u16) -> Result<usize> {
        for n in 1..=max_size {
            match self.read_u8().await? {
                b'\0' => {
                    self.skip(max_size - n).await?;
                    break;
                }
                c => buf.push(c as char),
            }
        }
        Ok(buf.len())
    }

    /// Consumes and discards exactly `bytes` bytes from the stream.
    async fn skip(&mut self, bytes: u16) -> Result<()> {
        let mut buf = vec![0_u8; bytes as usize];
        self.read_exact(&mut buf).await?;
        Ok(())
    }

    /// Reads a null-terminated string, stopping at the first `\0` or EOF.
    ///
    /// Unlike [`read_string`], this does not enforce a fixed field width -
    /// it reads until the stream ends or a null byte is encountered.
    async fn read_string_to_end(&mut self, buf: &mut String) -> Result<()> {
        let mut byte = [0u8; 1];
        loop {
            match self.read_exact(&mut byte).await {
                Ok(_) if byte[0] == 0 => break,
                Ok(_) => buf.push(byte[0] as char),
                Err(_) => break,
            }
        }
        Ok(())
    }

    /// Reads a Tibia 1.03 map position (X, Y only).
    ///
    /// Tibia 1.03 positions are two bytes - one for X and one for Y. The floor
    /// level (Z) is always 7 (ground floor) and is not encoded in the packet.
    async fn read_position(&mut self) -> Result<Position> {
        let x = self.read_u8().await? as u16;
        let y = self.read_u8().await? as u16;
        Ok(Position::new(x, y, 7))
    }

    /// Reads outfit colors from two packed bytes, plus one ignored padding byte.
    ///
    /// Tibia 1.03 packs four 4-bit color indices into two bytes:
    /// - Byte 1: legs (high nibble), shoes (low nibble)
    /// - Byte 2: head (high nibble), body (low nibble)
    async fn read_outfit_colors(&mut self) -> Result<OutfitColors> {
        let (legs, shoes) = self.read_u4().await?;
        let (head, body)  = self.read_u4().await?;
        let _ = self.read_u8().await?;
        Ok(OutfitColors::new(head, body, legs, shoes))
    }

    /// Reads a gender byte. `1` is [`Gender::Male`], anything else is [`Gender::Female`].
    async fn read_gender(&mut self) -> Result<Gender> {
        Ok(match self.read_u8().await? {
            1 => Gender::Male,
            _ => Gender::Female,
        })
    }
}

// Blanket impl so any AsyncWrite + Unpin automatically gets WriteExt.
impl<W: AsyncWrite + Unpin> WriteExt for W {}

/// Write helpers for encoding Tibia 1.03 network packets.
pub trait WriteExt: AsyncWrite + Unpin + Sized {
    /// Writes a Tibia 1.03 packet header followed by the packet type byte.
    ///
    /// All packets begin with four zero bytes (`u32_le = 0`) and then a
    /// one-byte packet identifier from [`PacketOut`].
    async fn write_packet(&mut self, packet: PacketOut) -> Result<()> {
        self.write_u32_le(0).await?;
        self.write_u8(packet as u8).await?;
        Ok(())
    }

    /// Packs two 4-bit values into a single byte and writes it.
    async fn write_u4(&mut self, high: u8, low: u8) -> Result<()> {
        self.write_u8((high << 4) + low).await?;
        Ok(())
    }

    /// Writes a string followed by a null terminator byte.
    async fn write_null_terminated_string(&mut self, s: &str) -> Result<()> {
        self.write_all(s.as_bytes()).await?;
        self.write_u8(0).await?;
        Ok(())
    }

    /// Writes a string into a fixed-width field, padding with trailing zeroes if needed.
    ///
    /// If `s` is longer than `length`, it is truncated. If shorter, the remaining
    /// bytes are zero-filled so the field width stays consistent.
    async fn write_fixed_string(&mut self, s: &str, length: u16) -> Result<()> {
        let len = length as usize;
        if s.len() >= len {
            self.write_all(&s.as_bytes()[..len]).await?;
        } else {
            self.write_all(s.as_bytes()).await?;
            self.write_zeroes(len - s.len()).await?;
        }
        Ok(())
    }

    /// Writes `count` zero bytes, used for padding and field alignment.
    async fn write_zeroes(&mut self, count: usize) -> Result<()> {
        self.write_all(&vec![0u8; count]).await?;
        Ok(())
    }

    /// Writes a Tibia 1.03 map position (X and Y only; Z is not encoded).
    async fn write_position(&mut self, pos: Position) -> Result<()> {
        self.write_u8(pos.x as u8).await?;
        self.write_u8(pos.y as u8).await?;
        Ok(())
    }

    /// Writes outfit colors as two packed bytes plus one padding byte.
    ///
    /// Mirrors the layout read by [`ReadExt::read_outfit_colors`]:
    /// - Byte 1: legs (high nibble), shoes (low nibble)
    /// - Byte 2: head (high nibble), body (low nibble)
    /// - Byte 3: `0x00` padding
    async fn write_outfit_colors(&mut self, outfit: OutfitColors) -> Result<()> {
        self.write_u4(outfit.legs, outfit.shoes).await?;
        self.write_u4(outfit.head, outfit.body).await?;
        self.write_u8(0).await?;
        Ok(())
    }

    /// Writes a gender byte. [`Gender::Male`] is `1`, [`Gender::Female`] is `0`.
    async fn write_gender(&mut self, gender: Gender) -> Result<()> {
        self.write_u8(match gender {
            Gender::Male   => 1,
            Gender::Female => 0,
        }).await?;
        Ok(())
    }
}
