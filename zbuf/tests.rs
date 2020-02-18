extern crate zbuf;

use std::mem::size_of;
use zbuf::*;

fn inline_capacity() -> usize {
    size_of::<usize>() + 4 + 4 - 1
}

macro_rules! common_tests {
    ($Buf: ident) => {
        use super::*;
        use std::ascii::AsciiExt;

        fn mutated<F: FnOnce(&mut $Buf)>(initial: &str, f: F) -> $Buf {
            let mut buf = from(initial);
            f(&mut buf);
            buf
        }

        #[test]
        fn new() {
            assert_eq!($Buf::new(), "");
            assert_eq!($Buf::new().len(), 0);
            assert_eq!($Buf::new().capacity(), inline_capacity());
        }

        #[test]
        fn with_capacity() {
            // Inline
            assert_eq!($Buf::with_capacity(8).capacity(), inline_capacity());
            assert_eq!(
                $Buf::with_capacity(inline_capacity()).capacity(),
                inline_capacity()
            );

            // Heap-allocated
            assert_eq!($Buf::with_capacity(inline_capacity() + 1).capacity(), 24); // (1 << 5) - 8
            assert_eq!($Buf::with_capacity(24).capacity(), 24); // (1 << 5) - 8
            assert_eq!($Buf::with_capacity(25).capacity(), 56); // (1 << 6) - 8
            assert_eq!($Buf::with_capacity(8000).capacity(), 8184); // (1 << 13) - 8

            assert_eq!(from("12345678901").capacity(), inline_capacity());
            assert_eq!(from("1234567890123456").capacity(), 24);

            assert_eq!($Buf::with_capacity(8), "");
            assert_eq!($Buf::with_capacity(8).len(), 0);
            assert_eq!($Buf::with_capacity(100), "");
            assert_eq!($Buf::with_capacity(100).len(), 0);
        }

        #[test]
        #[should_panic(expected = "overflow")]
        fn with_capacity_overflow() {
            $Buf::with_capacity(std::u32::MAX as usize);
        }

        #[test]
        fn len() {
            assert_eq!(from("").len(), 0);
            assert_eq!(from("1").len(), 1);
            assert_eq!(from("12345678901").len(), 11);
            assert_eq!(from("123456789012").len(), 12);
            assert_eq!(from("123456789012345").len(), 15);
            assert_eq!(from("1234567890123456").len(), 16);
        }

        #[test]
        fn pop_front() {
            assert_eq!(mutated("abcde", |b| b.pop_front(2)), "cde");
            assert_eq!(mutated("abcde", |b| b.pop_front(2)).len(), 3);
            assert_eq!(
                mutated("abcdefghijklmnopqrstuvwxyz", |b| b.pop_front(20)).len(),
                6
            );
        }

        #[test]
        #[should_panic]
        fn pop_front_out_of_bounds() {
            from("abcdefghijklmnopqrstuvwxyz").pop_front(30)
        }

        #[test]
        #[should_panic]
        fn pop_front_out_of_bounds_inline() {
            from("abcde").pop_front(10)
        }

        #[test]
        fn pop_back() {
            assert_eq!(mutated("abcde", |b| b.pop_back(2)), "abc");
            assert_eq!(
                mutated("abcdefghijklmnopqrstuvwxyz", |b| b.pop_back(20)),
                "abcdef"
            );
        }

        #[test]
        #[should_panic]
        fn pop_back_out_of_bounds() {
            from("abcdefghijklmnopqrstuvwxyz").pop_back(30)
        }

        #[test]
        #[should_panic]
        fn pop_back_out_of_bounds_inline() {
            from("abcde").pop_back(10)
        }

        #[test]
        fn split_off() {
            let mut buf = from("abcde");
            assert_eq!(buf.split_off(2), "cde");
            assert_eq!(buf, "ab");

            let mut buf = from("abcdefghijklmnopqrstuvwxyz");
            assert_eq!(buf.split_off(20), "uvwxyz");
            assert_eq!(buf, "abcdefghijklmnopqrst");
        }

        #[test]
        #[should_panic]
        fn split_off_out_of_bounds() {
            from("abcdefghijklmnopqrstuvwxyz").split_off(30);
        }

        #[test]
        #[should_panic]
        fn split_off_out_of_bounds_inline() {
            from("abcde").split_off(10);
        }

        #[test]
        fn clear() {
            assert_eq!(mutated("abcde", |b| b.clear()), "");
            assert_eq!(
                mutated("abcde", |b| b.clear()).capacity(),
                inline_capacity()
            );
            assert_eq!(mutated("abcdefghijklmnopqrstuvwxyz", |b| b.clear()), "");
            assert_eq!(
                mutated("abcdefghijklmnopqrstuvwxyz", |b| b.clear()).capacity(),
                56
            );
        }

        #[test]
        fn truncate() {
            assert_eq!(mutated("abcde", |b| b.truncate(100)), "abcde");
            assert_eq!(mutated("abcde", |b| b.truncate(3)), "abc");
            assert_eq!(
                mutated("1234567890123456", |b| b.truncate(100)),
                "1234567890123456"
            );
            assert_eq!(mutated("1234567890123456", |b| b.truncate(3)), "123");
        }

        #[test]
        fn reserve() {
            assert_eq!(mutated("", |b| b.reserve(10)).capacity(), inline_capacity());
            assert_eq!(mutated("", |b| b.reserve(20)).capacity(), 24);
            assert_eq!(
                mutated("1234567890123456", |b| b.reserve(10)).capacity(),
                56
            );
        }

        #[test]
        #[should_panic(expected = "overflow")]
        fn reserve_overflow() {
            $Buf::new().reserve(std::u32::MAX as usize)
        }

        #[test]
        fn write_to_uninitialized_tail() {
            let mut buf = from("hello");
            buf.reserve(10);
            unsafe {
                buf.write_to_uninitialized_tail(|uninitialized| {
                    for byte in &mut as_bytes_mut(uninitialized)[..3] {
                        *byte = b'!'
                    }
                    3
                })
            }
            assert_eq!(buf, "hello!!!");
        }

        #[test]
        #[should_panic]
        fn write_to_uninitialized_tail_out_of_bounds() {
            unsafe { $Buf::with_capacity(20).write_to_uninitialized_tail(|_| 25) }
        }

        #[test]
        fn write_to_zeroed_tail() {
            let mut buf = from("hello");
            buf.reserve(10);
            buf.write_to_zeroed_tail(|zeroed| {
                let bytes = unsafe { as_bytes_mut(zeroed) };
                assert!(bytes.iter().all(|&byte| byte == 0));
                for byte in &mut bytes[..3] {
                    *byte = b'!'
                }
                5
            });
            assert_eq!(buf, "hello!!!\0\0");
        }

        #[test]
        #[should_panic]
        fn write_to_zeroed_tail_out_of_bounds() {
            $Buf::with_capacity(20).write_to_zeroed_tail(|_| 25)
        }

        #[test]
        fn push_buf() {
            let mut buf = from("1234567890123456");
            buf.pop_front(2);
            assert_eq!(buf, "34567890123456");
            let buf2 = buf.split_off(10);
            assert_eq!(buf, "3456789012");
            assert_eq!(buf2, "3456");
            let address = buf.as_ptr() as usize;
            buf.push_buf(&buf2);
            assert_eq!(buf, "34567890123456");
            assert_eq!(buf2, "3456");
            assert_eq!(buf.as_ptr() as usize, address);
        }

        #[test]
        fn deref_mut_make_ascii_lowercase() {
            // K is a Kelvin sign. It maps to ASCII k in Unicode to_lowercase.
            assert_eq!(mutated("BK.z", |b| b.make_ascii_lowercase()), "bK.z");
        }
    };
}

mod bytes_buf {
    fn from(s: &str) -> BytesBuf {
        BytesBuf::from(s.as_bytes())
    }

    unsafe fn as_bytes_mut(s: &mut [u8]) -> &mut [u8] {
        s
    }

    common_tests!(BytesBuf);

    #[test]
    fn push_slice() {
        // Inline
        assert_eq!(mutated("abc", |b| b.push_slice(b"de")), "abcde");
        // Inline pushed into heap-allocated
        assert_eq!(
            mutated("1234567890", |b| b.push_slice(b"abcdefgh")),
            "1234567890abcdefgh"
        );
        // Heap-allocated
        assert_eq!(
            mutated("1234567890123456", |b| b.push_slice(b"ab")),
            "1234567890123456ab"
        );

        let mut buf = from("1234567890123456");
        let mut buf2 = buf.clone();
        buf.push_slice(b"ab");
        assert_eq!(buf, "1234567890123456ab");
        assert_eq!(buf2, "1234567890123456");
        buf2.push_slice(b"yz");
        assert_eq!(buf, "1234567890123456ab");
        assert_eq!(buf2, "1234567890123456yz");
    }

    #[test]
    fn read_into_unititialized_tail_from() {
        let mut file = std::fs::File::open(file!()).unwrap();
        let mut source = BytesBuf::with_capacity(file.metadata().unwrap().len() as usize);
        unsafe { while source.read_into_unititialized_tail_from(&mut file).unwrap() > 0 {} }
        // Self-referential test:
        assert!(StrBuf::from_utf8(source)
            .unwrap()
            .contains("This string is also unique"));
    }

    #[test]
    fn deref_mut() {
        assert_eq!(mutated("abc", |b| b[1] = b'-'), "a-c");

        let mut buf = from("1234567890123456");
        let mut buf2 = buf.clone();
        buf[12] = b'.';
        assert_eq!(buf, "123456789012.456");
        assert_eq!(buf2, "1234567890123456");
        buf2[2] = b'/';
        assert_eq!(buf, "123456789012.456");
        assert_eq!(buf2, "12/4567890123456");
    }

    #[test]
    fn debug() {
        assert_eq!(
            format!("{:?}", from("Yay 🎉!!")),
            r#"b"Yay \xF0\x9F\x8E\x89!!""#
        );
    }
}

mod str_buf {
    fn from(s: &str) -> StrBuf {
        StrBuf::from(s)
    }

    unsafe fn as_bytes_mut(s: &mut str) -> &mut [u8] {
        ::std::mem::transmute(s)
    }

    common_tests!(StrBuf);

    #[test]
    fn from_utf8() {
        let bytes_ok = BytesBuf::from(b"Yay \xF0\x9F\x8E\x89!!".as_ref());
        let bytes_err = BytesBuf::from(b"Yay \xF0\x9F\x8E\xFF!!".as_ref());
        assert_eq!(StrBuf::from_utf8(bytes_ok).unwrap(), "Yay 🎉!!");
        assert_eq!(
            StrBuf::from_utf8(bytes_err).unwrap_err().into_bytes_buf(),
            b"Yay \xF0\x9F\x8E\xFF!!"
        );
    }

    #[test]
    fn from_utf8_lossy() {
        let bytes_ok = BytesBuf::from(b"Yay \xF0\x9F\x8E\x89!!".as_ref());
        let bytes_err = BytesBuf::from(b"Yay \xF0\x9F\x8E\xFF!!".as_ref());
        assert_eq!(StrBuf::from_utf8_lossy(bytes_ok), "Yay 🎉!!");
        assert_eq!(StrBuf::from_utf8_lossy(bytes_err), "Yay ��!!");
    }

    #[test]
    fn push_str() {
        // Inline
        assert_eq!(mutated("abc", |b| b.push_str("de")), "abcde");
        // Inline pushed into heap-allocated
        assert_eq!(
            mutated("1234567890", |b| b.push_str("abcdefgh")),
            "1234567890abcdefgh"
        );
        // Heap-allocated
        assert_eq!(
            mutated("1234567890123456", |b| b.push_str("ab")),
            "1234567890123456ab"
        );

        let mut buf = from("1234567890123456");
        let mut buf2 = buf.clone();
        buf.push_str("ab");
        assert_eq!(buf, "1234567890123456ab");
        assert_eq!(buf2, "1234567890123456");
        buf2.push_str("yz");
        assert_eq!(buf, "1234567890123456ab");
        assert_eq!(buf2, "1234567890123456yz");
    }

    #[test]
    fn push_char() {
        // Inline
        assert_eq!(mutated("abc", |b| b.push_char('🎉')), "abc🎉");
        // Heap-allocated
        assert_eq!(
            mutated("1234567890123456", |b| b.push_char('🎉')),
            "1234567890123456🎉"
        );
    }

    #[test]
    fn debug() {
        assert_eq!(format!("{:?}", from("Yay 🎉!!")), r#""Yay 🎉!!""#);
    }
}

mod utf8_decoder {
    use super::*;

    #[test]
    fn lossy() {
        let chunks = [&[0xF0, 0x9F][..], &[0x8E], &[0x89, 0xF0, 0x9F]];
        let mut decoder = LossyUtf8Decoder::new();
        let mut bufs = Vec::new();
        for chunk in &chunks {
            bufs.extend(decoder.feed(BytesBuf::from(chunk)))
        }
        bufs.extend(decoder.end());
        let slices = bufs.iter().map(|b| &**b).collect::<Vec<&str>>();
        assert_eq!(slices, ["🎉", "�"]);
    }

    #[test]
    fn strict() {
        let chunks = [&[0xF0, 0x9F][..], &[0x8E], &[0x89, 0xF0, 0x9F], &[0x21]];
        let mut decoder = StrictUtf8Decoder::new();
        let mut results = Vec::new();
        for chunk in &chunks {
            results.extend(decoder.feed(BytesBuf::from(chunk)))
        }
        if let Err(error) = decoder.end() {
            results.push(Err(error))
        }
        let slices = results
            .iter()
            .map(|r| match *r {
                Ok(ref b) => Ok(&**b),
                Err(_) => Err(()),
            })
            .collect::<Vec<_>>();
        assert_eq!(slices, [Ok("🎉"), Err(()), Ok("!")]);
    }
}
