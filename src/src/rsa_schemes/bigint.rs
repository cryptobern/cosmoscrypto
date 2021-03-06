use core::slice;
use std::alloc::alloc_zeroed;
use std::mem::{MaybeUninit, self};
use std::ptr::{null, null_mut};

use derive::Serializable;
use gmp_mpfr_sys::gmp::{mpz_t, self};
use hex::FromHex;
use mcore::rand::RAND;
use rasn::types::BitString;
use rasn::{Encode, AsnType, Encoder, Decode};
use std::ffi::{CStr, c_void};
use std::fmt::Write;

use crate::rand::RNG;

#[macro_export] macro_rules! BIGINT {
    ($x:expr) => {
        BigInt::new_int($x as isize)
    };
}

#[macro_export] macro_rules! ZERO {
    () => {
        BigInt::new_int(0)
    };
}

#[macro_export] macro_rules! ONE {
    () => {
        BigInt::new_int(1)
    };
}

#[derive(Serializable)]
pub struct BigInt {
    value: MaybeUninit<mpz_t>
}

impl AsnType for BigInt {
    const TAG: rasn::Tag = rasn::Tag::BIT_STRING;
}

impl Encode for BigInt {
    fn encode_with_tag<E: Encoder>(&self, encoder: &mut E, tag: rasn::Tag) -> Result<(), E::Error> {
        encoder.encode_sequence(tag, |encoder| {
            self.to_bytes().encode(encoder)?;
            Ok(())
        })?;

        Ok(())
    }
}

impl Decode for BigInt {
    fn decode_with_tag<D: rasn::Decoder>(decoder: &mut D, tag: rasn::Tag) -> Result<Self, D::Error> {
        decoder.decode_sequence(tag, |sequence| {
            let mut bytes:Vec<u8> = Vec::<u8>::decode(sequence)?.into();
            Ok(Self::from_bytes(&mut bytes))
        })
    }
}

impl PartialEq for BigInt {
    fn eq(&self, other: &Self) -> bool {
        self.equals(&other)
    }
}

impl BigInt {
    pub fn new() -> Self {
        unsafe {
            let mut z = MaybeUninit::uninit();
            gmp::mpz_init(z.as_mut_ptr());
            Self {value: z }
        }
    }

    pub fn new_int(i: isize) -> Self {
        unsafe {
            let mut z = MaybeUninit::uninit();
            gmp::mpz_init(z.as_mut_ptr());
            gmp::mpz_set_si(z.as_mut_ptr(), i as i64);
            Self {value: z }
        }
    }

    pub fn new_copy(x: &Self) -> Self {
        unsafe {
            let mut z = MaybeUninit::uninit();
            gmp::mpz_init_set(z.as_mut_ptr(), x.value.as_ptr());
            Self { value: z }
        }
    }

    pub fn new_rand(rng: &mut RNG, bits: usize) -> Self {
        let mut g = Self::new();
        g.rand(rng, bits);
        g
    }

    pub fn rand(&mut self, rng: &mut RNG, bits: usize) {
        unsafe {
            let bytelen = f64::floor(bits as f64/8 as f64) as usize;
            let rem = bits%8;

            let mut s = String::with_capacity(bytelen + rem + 1);

            if rem != 0 {
                let mut mask: u8 = 0;
                let mut byte = rng.getbyte();
                for i in 0..rem {
                    mask += 1 << i;

                    if i == rem-1 {
                        byte |= 1 << i;
                    }
                }   
                
                byte &= mask;
                write!(&mut s, "{:02X}", byte).expect("Unable to get random bytes!");
            }

            for i in 0..bytelen {
                let mut byte = rng.getbyte();
                if i == 0 && rem == 0{
                    byte |= 1 << 7;
                }
                
                write!(&mut s, "{:02X}", byte).expect("Unable to get random bytes!");
            }
            write!(&mut s, "\0").expect("Unable to null terminate string");
            gmp::mpz_set_str(self.value.as_mut_ptr(), s.as_ptr() as *const i8, 16);
        }
    }

    pub fn new_prime(rng: &mut RNG, len: usize) -> Self {
        let mut x = BigInt::new();

        loop {
            x.rand(rng, len);

            if x.is_prime() {
                break;
            }
        } 
        
        x
    }

    pub fn cmp(&self, y: &Self) -> i32 {
        unsafe {
            let res = gmp::mpz_cmp(self.value.as_ptr(), y.value.as_ptr());
            res
        }
    }

    pub fn set(&mut self, y: &Self) {
        unsafe {
            gmp::mpz_set(self.value.as_mut_ptr(), y.value.as_ptr());
        }
    }

    pub fn add(&self, y:&Self) -> Self {
        unsafe {
            let mut z = MaybeUninit::uninit();
            gmp::mpz_init(z.as_mut_ptr());
            gmp::mpz_add(z.as_mut_ptr(), self.value.as_ptr(), y.value.as_ptr());
            Self { value: z }
        }
    }

    pub fn inc(&self, k: u64) -> Self {
        unsafe {
            let mut z = MaybeUninit::uninit();
            gmp::mpz_init(z.as_mut_ptr());
            gmp::mpz_add_ui(z.as_mut_ptr(), self.value.as_ptr(), k);
            Self { value: z }
        }
    }

    pub fn sub(&self, y:&Self) -> Self {
        unsafe {
            let mut z = MaybeUninit::uninit();
            gmp::mpz_init(z.as_mut_ptr());
            gmp::mpz_sub(z.as_mut_ptr(), self.value.as_ptr(), y.value.as_ptr());
            Self { value: z }
        }
    }

    pub fn dec(&self, k: u64) -> Self {
        unsafe {
            let mut z = MaybeUninit::uninit();
            gmp::mpz_init(z.as_mut_ptr());
            gmp::mpz_sub_ui(z.as_mut_ptr(), self.value.as_ptr(), k);
            Self { value: z }
        }
    }

    pub fn mul(&self, y:&Self) -> Self {
        unsafe {
            let mut z = MaybeUninit::uninit();
            gmp::mpz_init(z.as_mut_ptr());
            gmp::mpz_mul(z.as_mut_ptr(), self.value.as_ptr(), y.value.as_ptr());
            Self { value: z }
        }
    }

    pub fn rmod(&self, m:&Self) -> Self {
        unsafe {
            let mut z = MaybeUninit::uninit();
            gmp::mpz_init(z.as_mut_ptr());
            gmp::mpz_mod(z.as_mut_ptr(), self.value.as_ptr(), m.value.as_ptr());
            Self { value: z }
        }
    }

    pub fn mul_mod(&self, y:&Self, m:&Self) -> Self {
        unsafe {
            let mut z = MaybeUninit::uninit();
            gmp::mpz_init(z.as_mut_ptr());
            gmp::mpz_mul(z.as_mut_ptr(), self.value.as_ptr(), y.value.as_ptr()); 
            gmp::mpz_mod(z.as_mut_ptr(), z.as_ptr(), m.value.as_ptr());
            Self { value: z }
        }
    }

    pub fn pow(&self, y: u64) -> Self {
        unsafe {
            let mut z = MaybeUninit::uninit();
            gmp::mpz_init(z.as_mut_ptr());
            gmp::mpz_pow_ui(z.as_mut_ptr(), self.value.as_ptr(), y as u64);
            Self { value: z }
        }
    }

    pub fn pow_mod(&self, e:&Self, m:&Self) -> Self {
        unsafe {
            let mut z = MaybeUninit::uninit();
            gmp::mpz_init(z.as_mut_ptr());
            gmp::mpz_powm(z.as_mut_ptr(), self.value.as_ptr(), e.value.as_ptr(), m.value.as_ptr());
            Self { value: z }
        }
    }

    pub fn root(&mut self, n: u64) -> Self {
        unsafe {
            let mut z = MaybeUninit::uninit();
            gmp::mpz_init(z.as_mut_ptr()); 
            gmp::mpz_rootrem(self.value.as_mut_ptr(), z.as_mut_ptr(), self.value.as_ptr(), n as u64);
            Self { value: z }
        }
    }

    pub fn inv_mod(&self, m:&Self) -> Self {
        unsafe {
            let mut z = MaybeUninit::uninit();
            gmp::mpz_init(z.as_mut_ptr());
            gmp::mpz_invert(z.as_mut_ptr(), self.value.as_ptr(), m.value.as_ptr());
            Self { value: z }
        }
    }

    pub fn equals(&self, y:&Self) -> bool {
        self.cmp(&y) == 0
    }

    pub fn imul(&self, i: isize) -> Self {
        unsafe {
            let mut z = MaybeUninit::uninit();
            gmp::mpz_init(z.as_mut_ptr());  
            gmp::mpz_mul_si(z.as_mut_ptr(), self.value.as_ptr(), i as i64);
            
            Self { value:z }
        }
    }

    pub fn is_prime(&self) -> bool {
        unsafe {
            gmp::mpz_probab_prime_p(self.value.as_ptr(), 45) != 0
        }
    }

    pub fn is_even(&self) -> bool {
        unsafe {
            gmp::mpz_even_p(self.value.as_ptr()) != 0
        }
    }

    pub fn jacobi(x: &Self, y:&Self) -> isize {
        unsafe {
            gmp::mpz_jacobi(x.value.as_ptr(), y.value.as_ptr()) as isize
        }
    }

    pub fn coprime(&self, i:isize) -> bool {
        unsafe {
            let x = BigInt::new_int(i);
            let mut y = BigInt::new();
            gmp::mpz_gcd(y.value.as_mut_ptr(), self.value.as_ptr(), x.value.as_ptr());
            y.equals(&BigInt::new_int(1))
        }
    }

    pub fn div(&self, y: &Self) -> Self {
        unsafe {
            let mut z = MaybeUninit::uninit();
            gmp::mpz_init(z.as_mut_ptr());          
            gmp::mpz_fdiv_q(z.as_mut_ptr(), self.value.as_ptr(), y.value.as_ptr());
            Self { value: z }
        }
    }

    pub fn legendre(&self, y: &Self) -> isize {
        unsafe {
            gmp::mpz_legendre(self.value.as_ptr(), y.value.as_ptr()) as isize
        }
    }

    pub fn to_string(&self) -> String {
        unsafe {
            let str = gmp::mpz_get_str(std::ptr::null_mut(), 16, self.value.as_ptr());
            let s:String = CStr::from_ptr(str).to_str().unwrap().to_string();
            s
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let size:usize = 0;
        let size_ptr = &size as *const usize;

        unsafe {
            let bytes_ptr = gmp::mpz_export(null_mut(), size_ptr as *mut usize, 1, 1, 1, 0, self.value.as_ptr()) as *mut u8;
            let bytes:Vec<u8> = slice::from_raw_parts(bytes_ptr, size).to_vec();
            bytes
        }
    }

    pub fn from_bytes(bytes: &mut [u8]) -> Self {
        unsafe {
            let mut z = MaybeUninit::uninit();
            let op: *const c_void = bytes.as_ptr() as *const c_void;
            gmp::mpz_init(z.as_mut_ptr()); 
            gmp::mpz_import(z.as_mut_ptr(), bytes.len(), 1, 1, 1, 0, op);
            Self { value: z }
        }
    }
}

impl Clone for BigInt {
    fn clone(&self) -> Self {
        BigInt::new_copy(&self)
    }
}