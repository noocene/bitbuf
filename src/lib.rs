#![no_std]

use core::{
    borrow::{Borrow, BorrowMut},
    mem::replace,
};

#[derive(Debug)]
pub struct Insufficient;

#[derive(Debug)]
pub struct Overflow;

#[derive(Debug)]
pub enum UnalignedError {
    Insufficient(Insufficient),
    Overflow(Overflow),
}

impl From<Insufficient> for UnalignedError {
    fn from(inwrite: Insufficient) -> Self {
        UnalignedError::Insufficient(inwrite)
    }
}

pub struct Fill<T: BorrowMut<[u8]>> {
    len: usize,
    buf: T,
}

pub struct CappedFill<T: BorrowMut<[u8]>> {
    len: usize,
    cap: usize,
    buf: T,
}

impl<T: BorrowMut<[u8]>> CappedFill<T> {
    pub fn into_inner(self) -> T {
        self.buf
    }

    pub fn fill_from<B: BitBuf>(&mut self, mut buf: B) -> Result<(), Insufficient> {
        let target_buf = self.buf.borrow_mut();
        let mut target = BitSliceMut::new(target_buf);
        target.advance(self.len).unwrap();

        loop {
            if self.len < self.cap {
                if buf.remaining() >= 8 && self.cap - self.len >= 8 {
                    let byte = buf.read_byte().unwrap();
                    target.write_byte(byte).unwrap();
                    self.len += 8;
                } else {
                    let bit = buf.read_bool();
                    match bit {
                        Ok(bit) => {
                            target.write_bool(bit).unwrap();
                            self.len += 1
                        }
                        Err(e) => return Err(e),
                    }
                }
            } else {
                return Ok(());
            }
        }
    }

    pub fn as_buf<'a>(&'a self) -> impl BitBuf + 'a {
        BitSlice::new(self.buf.borrow())
    }

    pub fn new(mut buf: T, cap: usize) -> Result<Self, Overflow> {
        if cap > buf.borrow_mut().len() * 8 {
            return Err(Overflow);
        }
        Ok(CappedFill { len: 0, buf, cap })
    }
}

impl<T: BorrowMut<[u8]>> Fill<T> {
    pub fn into_inner(self) -> T {
        self.buf
    }

    pub fn fill_from<B: BitBuf>(&mut self, mut buf: B) -> Result<(), Insufficient> {
        let target_buf = self.buf.borrow_mut();
        let buf_len = target_buf.len() * 8;
        let mut target = BitSliceMut::new(target_buf);
        target.advance(self.len).unwrap();

        loop {
            if self.len < buf_len {
                if buf.remaining() >= 8 && buf_len - self.len >= 8 {
                    let byte = buf.read_byte().unwrap();
                    target.write_byte(byte).unwrap();
                    self.len += 8;
                } else {
                    let bit = buf.read_bool();
                    match bit {
                        Ok(bit) => {
                            target.write_bool(bit).unwrap();
                            self.len += 1
                        }
                        Err(e) => return Err(e),
                    }
                }
            } else {
                return Ok(());
            }
        }
    }

    pub fn as_buf<'a>(&'a self) -> impl BitBuf + 'a {
        BitSlice::new(self.buf.borrow())
    }

    pub fn new(buf: T) -> Self {
        Fill { len: 0, buf }
    }
}

pub struct Drain<T: Borrow<[u8]>> {
    len: usize,
    buf: T,
}

impl<T: Borrow<[u8]>> Drain<T> {
    pub fn into_inner(self) -> T {
        self.buf
    }

    pub fn drain_into<B: BitBufMut>(&mut self, mut buf: B) -> Result<(), Insufficient> {
        let target_buf = self.buf.borrow();
        let buf_len = target_buf.len() * 8;
        let mut target = BitSlice::new(target_buf);
        target.advance(self.len).unwrap();

        loop {
            if self.len < buf_len {
                if buf.remaining() >= 8 && buf_len - self.len >= 8 {
                    buf.write_byte(target.read_byte().unwrap()).unwrap();
                    self.len += 8;
                } else {
                    let bit = buf.write_bool(target.read_bool().unwrap());
                    if let Ok(()) = bit {
                        self.len += 1;
                    } else {
                        return Err(Insufficient);
                    }
                }
            } else {
                return Ok(());
            }
        }
    }

    pub fn as_buf<'a>(&'a self) -> impl BitBuf + 'a {
        BitSlice::new(self.buf.borrow())
    }

    pub fn new(buf: T) -> Self {
        Drain { len: 0, buf }
    }
}

pub struct CappedDrain<T: Borrow<[u8]>> {
    cap: usize,
    len: usize,
    buf: T,
}

impl<T: Borrow<[u8]>> CappedDrain<T> {
    pub fn into_inner(self) -> T {
        self.buf
    }

    pub fn drain_into<B: BitBufMut>(&mut self, mut buf: B) -> Result<(), Insufficient> {
        let target_buf = self.buf.borrow();
        let mut target = BitSlice::new(target_buf);
        target.advance(self.len).unwrap();

        loop {
            if self.len < self.cap {
                if buf.remaining() >= 8 && self.cap - self.len >= 8 {
                    buf.write_byte(target.read_byte().unwrap()).unwrap();
                    self.len += 8;
                } else {
                    let bit = buf.write_bool(target.read_bool().unwrap());
                    if let Ok(()) = bit {
                        self.len += 1;
                    } else {
                        return Err(Insufficient);
                    }
                }
            } else {
                return Ok(());
            }
        }
    }

    pub fn as_buf<'a>(&'a self) -> impl BitBuf + 'a {
        BitSlice::new(self.buf.borrow())
    }

    pub fn new(buf: T, cap: usize) -> Result<Self, Overflow> {
        if cap > buf.borrow().len() * 8 {
            return Err(Overflow);
        }
        Ok(CappedDrain { len: 0, buf, cap })
    }
}

pub trait BitBuf {
    fn advance(&mut self, bits: usize) -> Result<(), Insufficient>;
    fn read_all(&mut self, dst: &mut [u8], bits: usize) -> Result<(), UnalignedError> {
        if self.remaining() < bits {
            Err(UnalignedError::Insufficient(Insufficient))
        } else {
            self.read(dst, bits)
                .map_err(UnalignedError::Overflow)
                .map(|_| {})
        }
    }
    fn read(&mut self, dst: &mut [u8], bits: usize) -> Result<usize, Overflow>;
    fn read_aligned(&mut self, dst: &mut [u8]) -> usize {
        self.read(dst, dst.len() * 8)
            .expect("overflowed aligned slice")
    }
    fn read_aligned_all(&mut self, dst: &mut [u8]) -> Result<(), Insufficient> {
        self.read_all(dst, dst.len() * 8).map_err(|e| match e {
            UnalignedError::Insufficient(e) => e,
            UnalignedError::Overflow(_) => panic!("overflowed aligned slice"),
        })
    }
    fn read_bool(&mut self) -> Result<bool, Insufficient>;
    fn read_byte(&mut self) -> Result<u8, Insufficient> {
        let mut data = [0u8];
        self.read_aligned_all(&mut data)?;
        Ok(data[0])
    }
    fn remaining(&self) -> usize;
    fn len(&self) -> usize;
}

impl<'a, T: ?Sized + BitBuf> BitBuf for &'a mut T {
    fn advance(&mut self, bits: usize) -> Result<(), Insufficient> {
        T::advance(self, bits)
    }
    fn read_all(&mut self, dst: &mut [u8], bits: usize) -> Result<(), UnalignedError> {
        T::read_all(self, dst, bits)
    }
    fn read(&mut self, dst: &mut [u8], bits: usize) -> Result<usize, Overflow> {
        T::read(self, dst, bits)
    }
    fn read_aligned(&mut self, dst: &mut [u8]) -> usize {
        T::read_aligned(self, dst)
    }
    fn read_aligned_all(&mut self, dst: &mut [u8]) -> Result<(), Insufficient> {
        T::read_aligned_all(self, dst)
    }
    fn read_bool(&mut self) -> Result<bool, Insufficient> {
        T::read_bool(self)
    }
    fn read_byte(&mut self) -> Result<u8, Insufficient> {
        T::read_byte(self)
    }
    fn remaining(&self) -> usize {
        T::remaining(self)
    }
    fn len(&self) -> usize {
        T::len(self)
    }
}

impl<'a, T: ?Sized + BitBufMut> BitBufMut for &'a mut T {
    fn advance(&mut self, bits: usize) -> Result<(), Insufficient> {
        T::advance(self, bits)
    }
    fn write_all(&mut self, dst: &[u8], bits: usize) -> Result<(), UnalignedError> {
        T::write_all(self, dst, bits)
    }
    fn write(&mut self, dst: &[u8], bits: usize) -> Result<usize, Overflow> {
        T::write(self, dst, bits)
    }
    fn write_aligned(&mut self, dst: &[u8]) -> usize {
        T::write_aligned(self, dst)
    }
    fn write_aligned_all(&mut self, dst: &[u8]) -> Result<(), Insufficient> {
        T::write_aligned_all(self, dst)
    }
    fn write_bool(&mut self, bit: bool) -> Result<(), Insufficient> {
        T::write_bool(self, bit)
    }
    fn write_byte(&mut self, byte: u8) -> Result<(), Insufficient> {
        T::write_byte(self, byte)
    }
    fn remaining(&self) -> usize {
        T::remaining(self)
    }
    fn len(&self) -> usize {
        T::len(self)
    }
}

#[derive(Debug)]
pub struct BitSlice<'a> {
    data: &'a [u8],
    prefix: u8,
    len: usize,
}

impl<'a> BitBuf for BitSlice<'a> {
    fn advance(&mut self, bits: usize) -> Result<(), Insufficient> {
        if bits > self.remaining() {
            return Err(Insufficient);
        }
        self.prefix += (bits & 7) as u8;
        if self.prefix >= 8 {
            self.prefix -= 8;
            self.data = self.data.get((bits / 8) + 1..).ok_or(Insufficient)?;
        } else {
            self.data = self.data.get(bits / 8..).ok_or(Insufficient)?;
        }
        self.len += bits;
        Ok(())
    }

    fn read_aligned(&mut self, dst: &mut [u8]) -> usize {
        let re = self.remaining();
        let len = dst.len();
        let len = if len * 8 > re { re } else { len };
        if len & 7 != 0 {
            return self.read(dst, len * 8).expect("overflowed aligned slice");
        } else {
            for i in 0..dst.len() {
                dst[i] = self.byte_at_offset(i * 8).unwrap();
            }
        }
        len
    }

    fn len(&self) -> usize {
        self.len
    }

    fn read(&mut self, dst: &mut [u8], bits: usize) -> Result<usize, Overflow> {
        let re = self.remaining();
        let bits = if bits > re { re } else { bits };
        let bytes = bits / 8;
        let len = dst.len();
        if len * 8 < bits {
            return Err(Overflow);
        }
        for i in 0..bytes {
            dst[i] = self
                .byte_at_offset(i * 8)
                .map_err(UnalignedError::Insufficient)
                .unwrap();
        }
        let rem = bits & 7;
        if rem != 0 {
            if len < bytes + 1 {
                return Err(Overflow);
            }
            let byte = self
                .data_at_offset(bytes * 8, rem)
                .map_err(UnalignedError::Insufficient)
                .unwrap()
                & (255 << (8 - rem));
            dst[bytes] |= byte;
            dst[bytes] &= byte;
        }
        self.advance(bits).unwrap();
        Ok(bits)
    }

    fn read_bool(&mut self) -> Result<bool, Insufficient> {
        let byte = self.data_at_offset(0, 1)?;
        self.advance(1).unwrap();
        Ok(byte & 128 != 0)
    }

    fn read_byte(&mut self) -> Result<u8, Insufficient> {
        let byte = self.byte_at_offset(0)?;
        self.advance(8).unwrap();
        Ok(byte)
    }

    fn remaining(&self) -> usize {
        self.data.len() * 8 - self.prefix as usize
    }
}

impl<'a> BitSlice<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        BitSlice {
            data,
            prefix: 0,
            len: 0,
        }
    }

    fn data_at_offset(&self, offset: usize, size: usize) -> Result<u8, Insufficient> {
        let len = self.remaining();
        let offset = self.prefix as usize + offset;
        if offset == 0 {
            if len == 0 {
                return Err(Insufficient);
            }
            Ok(self.data[0])
        } else if len < size {
            Err(Insufficient)
        } else {
            let offset_bytes = offset / 8;
            let offset_rem = offset & 7;
            if offset_rem == 0 {
                Ok(self.data[offset_bytes])
            } else {
                let offset_rem_inv = 8 - offset_rem;
                Ok(if size + offset_rem <= 8 {
                    ((self.data[offset_bytes] & (255 >> offset_rem)) << offset_rem)
                } else {
                    ((self.data[offset_bytes] & (255 >> offset_rem)) << offset_rem)
                        + ((self.data[(offset_bytes) + 1] & (255 << offset_rem_inv))
                            >> offset_rem_inv)
                })
            }
        }
    }

    fn byte_at_offset(&self, offset: usize) -> Result<u8, Insufficient> {
        self.data_at_offset(offset, 8)
    }
}

#[derive(Debug)]
pub struct BitSliceMut<'a> {
    data: &'a mut [u8],
    prefix: u8,
    len: usize,
}

pub trait BitBufMut {
    fn len(&self) -> usize;
    fn remaining(&self) -> usize;
    fn advance(&mut self, bits: usize) -> Result<(), Insufficient>;
    fn write_bool(&mut self, item: bool) -> Result<(), Insufficient>;
    fn write(&mut self, data: &[u8], bits: usize) -> Result<usize, Overflow>;
    fn write_all(&mut self, data: &[u8], bits: usize) -> Result<(), UnalignedError> {
        if bits > self.remaining() {
            Err(UnalignedError::Insufficient(Insufficient))
        } else {
            self.write(data, bits)
                .map_err(UnalignedError::Overflow)
                .map(|_| {})
        }
    }
    fn write_byte(&mut self, data: u8) -> Result<(), Insufficient> {
        self.write_aligned_all(&[data])
    }
    fn write_aligned(&mut self, data: &[u8]) -> usize {
        self.write(data, data.len() * 8)
            .expect("overflowed aligned buffer")
    }
    fn write_aligned_all(&mut self, data: &[u8]) -> Result<(), Insufficient> {
        let bits = data.len() * 8;
        if bits > self.remaining() {
            Err(Insufficient)
        } else {
            self.write(data, bits).expect("overflowed aligned buffer");
            Ok(())
        }
    }
}

impl<'a> BitBufMut for BitSliceMut<'a> {
    fn len(&self) -> usize {
        self.len
    }

    fn remaining(&self) -> usize {
        self.data.len() * 8 - self.prefix as usize
    }

    fn advance(&mut self, bits: usize) -> Result<(), Insufficient> {
        if bits > self.remaining() {
            return Err(Insufficient);
        }
        self.prefix += (bits & 7) as u8;
        if self.prefix >= 8 {
            self.prefix -= 8;
            let empty = &mut [];
            let data = replace(&mut self.data, empty);
            self.data = data.get_mut((bits / 8) + 1..).ok_or(Insufficient)?;
        } else {
            let empty = &mut [];
            let data = replace(&mut self.data, empty);
            self.data = data.get_mut(bits / 8..).ok_or(Insufficient)?;
        }
        self.len += bits;
        Ok(())
    }

    fn write_bool(&mut self, item: bool) -> Result<(), Insufficient> {
        if self.data.len() == 0 {
            return Err(Insufficient);
        }
        let byte = &mut self.data[0];
        if item {
            *byte |= 128 >> self.prefix;
        } else {
            *byte &= 255 ^ (128 >> self.prefix);
        }
        self.advance(1)?;
        Ok(())
    }

    fn write_byte(&mut self, item: u8) -> Result<(), Insufficient> {
        if self.data.len() == 0 {
            return Err(Insufficient.into());
        }
        if self.prefix == 0 {
            self.data[0] = item;
        } else {
            if self.data.len() == 1 {
                return Err(Insufficient.into());
            }
            let inv_prefix = 8 - self.prefix;
            self.data[0] |= item >> self.prefix;
            self.data[0] &= (item >> self.prefix) | (255 << inv_prefix);
            self.data[1] |= item << inv_prefix;
            self.data[1] &= (item << inv_prefix) | (255 << self.prefix);
        }
        self.advance(8)?;
        Ok(())
    }

    fn write(&mut self, data: &[u8], bits: usize) -> Result<usize, Overflow> {
        let bytes = bits / 8;
        let re = self.remaining();
        let bits = if bits > re { re } else { bits };
        if bits == 0 {
            return Ok(0);
        }
        let len = data.len();
        if len == 0 {
            return Err(Overflow);
        }
        if len * 8 < bits {
            return Err(Overflow);
        }
        for i in 0..bytes {
            self.write_byte(data[i])
                .expect("overflowed restricted buffer");
        }
        let rem = bits & 7;
        if rem != 0 {
            if len < bytes + 1 {
                return Err(Overflow);
            }
            let byte = data[bytes];
            self.write(byte, rem).expect("overflowed restricted buffer");
        }
        Ok(bits)
    }

    fn write_aligned(&mut self, data: &[u8]) -> usize {
        let len = data.len() * 8;
        let re = self.remaining();
        let len = if len > re { re } else { len };
        if len & 7 != 0 {
            BitBufMut::write(self, data, len).expect("overflowed aligned buffer");
        } else {
            for byte in &data[..len / 8] {
                self.write_byte(*byte).expect("overflowed aligned buffer");
            }
        }
        len
    }
}

impl<'a> BitSliceMut<'a> {
    pub fn new(data: &'a mut [u8]) -> Self {
        BitSliceMut {
            data,
            prefix: 0,
            len: 0,
        }
    }

    fn write(&mut self, item: u8, bits: usize) -> Result<(), Insufficient> {
        if self.prefix == 0 {
            let inv_bits = 8 - bits;
            let litem = item & (255 << (inv_bits));
            let hitem = item | (255 >> (inv_bits));
            self.data[0] |= litem;
            self.data[0] &= hitem;
        } else {
            let inv_bits = 8 - bits;
            let inv_prefix = 8 - self.prefix;
            let litem = item & (255 << inv_bits);
            let hitem = item | (255 >> inv_bits);
            self.data[0] |= litem >> self.prefix;
            self.data[0] &= (hitem >> self.prefix) | (255 << inv_prefix);
            self.data[1] |= litem << inv_prefix;
            self.data[1] &= (hitem << inv_prefix) | (255 << self.prefix);
        }
        self.advance(bits)?;
        Ok(())
    }
}
