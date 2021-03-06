# rel-ptr

`rel-ptr` a library for relative pointers, which can be used to create
moveable self-referential types. This library was inspired by
Johnathan Blow's work on Jai, where he added relative pointers
as a primitive into Jai.

A relative pointer is a pointer that uses an offset and it's current location to
calculate where it points to.

Minimum Rust Version = 1.36.0

## Safety

See the `RelPtr` type docs for safety information

## Features

### `no_std`

This crate is `no-std` compatible, simply add the feature `no_std` to move into `no_std` mode.

### nightly

with nightly you get the ability to use trait objects with relative pointers

## Example

take the memory segment below

`[.., 0x3a, 0x10, 0x02, 0xe4, 0x2b ..]`

where `0x3a` has the address `0xff304050` (32-bit system)
then `0x2b` has the address `0xff304054`.

if we have a 1-byte relative pointer (`RelPtr<_, i8>`)
at the address `0xff304052`, then that relative pointer points to
`0x2b` as well, this is because its address `0xff304052`, plus its
offset, `0x02` points to `0x2b`.

There are three interesting things
about this
1) it only took 1 byte to point to another value,
2) a relative pointer cannot access all memory, only memory near it
3) if both the relative pointer and the pointee move together,
   then the relative pointer will not be invalidated

The third point is what makes moveable self-referential structs possible

The type `RelPtr<T, I>` is a relative pointer. `T` is what it points to,
and `I` is what it uses to store its offset. In practice you can ignore `I`,
which is defaulted to `isize`, because that will cover all of your cases for using
relative pointers. But if you want to optimize the size of the pointer, you can use
any type that implements `Delta`. Some types from std that do so are:
`i8`, `i16`, `i32`, `i64`, `i128`, and `isize`. Note that the trade off is that as you
decrease the size of the offset, you decrease the range to which you can point to.
`isize` will cover at least half of addressable memory, so it should work unless you do
something really crazy. For self-referential structs use a type whose max value is atleast
as big as your struct. i.e. `std::mem::size_of::<T>() <= I::max_value()`.

Note on usized types: these are harder to get working 

## Self Referential Type Example

```rust
 struct SelfRef {
     value: (String, u32),
     ptr: RelPtr<String, i8>
 }

 impl SelfRef {
     pub fn new(s: String, i: u32) -> Self {
         let mut this = Self {
             value: (s, i),
             ptr: RelPtr::null()
         };
         
         this.ptr.set(&mut this.value.0).unwrap();
         
         this
     }

     pub fn fst(&self) -> &str {
         unsafe { self.ptr.as_ref_unchecked() }
     }

     pub fn snd(&self) -> u32 {
         self.value.1
     }
 }

 let s = SelfRef::new("Hello World".into(), 10);
 
 assert_eq!(s.fst(), "Hello World");
 assert_eq!(s.snd(), 10);
 
 let s = Box::new(s); // force a move, note: relative pointers even work on the heap
 
 assert_eq!(s.fst(), "Hello World");
 assert_eq!(s.snd(), 10);
```

This example is contrived, and only useful as an example.
In this example, we can see a few important parts to safe moveable self-referential types,
lets walk through them.

First, the definition of `SelfRef`, it contains a value and a relative pointer, the relative pointer that will point into the tuple inside of `SelfRef.value` to the `String`. There are no lifetimes involved because they would either make `SelfRef` immovable, or they could not be resolved correctly.

We see a pattern inside of `SelfRef::new`, first create the object, and use the sentinel `RelPtr::null()` and immediately afterwards assigning it a value using `RelPtr::set` and unwraping the result. This unwrapping is get quick feedback on whether or not the pointer was set, if it wasn't set then we can increase the size of the offset and resolve that.

Once the pointer is set, moving the struct is still safe because it is using a *relative* pointer, so it doesn't matter where it is, only it's offset from its pointee.
In `SelfRef::fst` we use `RelPtr::as_ref_unchecked` because it is impossible to invalidate the pointer. It is impossible because we cannot
set the relative pointer directly, and we cannot change the offsets of the fields of `SelfRef` after the relative pointer is set.

---

# License

<sup>
Licensed under either of <a href="APACHE-LICENSE">Apache License, Version
2.0</a> or <a href="MIT-LICENSE">MIT license</a> at your option.
</sup>

<br>

<sub>
Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in `rel-ptr` by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
</sub>

---
# Release Notes

## 0.2.4

### Changes

 * Chaned to `std::mem::MaybeUnint` from a custom version because it is stable now on rustc version 1.36.0

## 0.2.3

### Changes

 * Moved `NonZero*` out of nightly due to rustc version 1.34.0

## 0.2.2

### Removals

 * dependency to `unreachable`
    * unnecessary in the presence of `std::hint::unreachable`, also it is safer to use the `std` version because `std` version is guarenteed to be optimized away
    * has a different behaviour on debug mode than I want, new impl panics on debug mode and is optimized away on release mode

## 0.2.1

### Additions

 * Documentation on `Nullable` and how it plays with `Delta`

### Changes
 
 * Fixed mutability bug, getting a raw ptr (`*mut T`) or a non-nullable ptr (`NonNull<T>`) should require a unique lock on `RelPtr`


## 0.2.0

### Additions

 * Added constructors on `TraitObject`, now there is `from_ref` and `from_mut` to allow easier transitions to and from `TraitObject`
 * More documentation

### Removals

 * `Default` bound for `MetaData::Data`
    * It is now UB to access `MetaData::Data` before the relative pointer is set
 * `TraitObject::new` in favor of the new constructors

### Changes

 * Reworked `MetaData::decompose`
    * Chagned to `MetaData::data`, the pointer can be extracted via pointer casts, so only data was needed
 * Converted `MetaData` to use `std::mem::NonNull` as it is easier to work with
    * This is due to using `Option<NonNull<T>>` allows representing null even if `T: !Sized`

### Notes

I am not anticipating any more large scale changes to the api, so this should be as the final api. I will wait and see if any there are any bugs, before releasing 1.0.0. I will also have to wait on the results of [this](https://github.com/rust-lang/unsafe-code-guidelines/issues/97) github discussion around the layouts of types, as it relates to how safe this model of moveable self-referential types are.

## 0.1.4

### Additions

 * Support for `NonZero*` integers
 * Formatting for all `RelPtr` whose idicies support formatting
 
### Changes

 * Converted api to use `&mut T` instead of `&T`
    * this better represents the semantics of `RelPtr` and was suggested by [Yandros](https://users.rust-lang.org/u/Yandros)
 * Moved `Delta::ZERO` to `Nullable::NULL`
    * This was to enable support for `NonZero*` types
 * Updated documentation to better explain possible UB

 * Changed from `TraitObject::into` to `TraitObject::as_ref` and `TraitObject::as_mut`
