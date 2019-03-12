#![cfg_attr(feature = "no_std", no_std)]
#![cfg_attr(feature = "nightly", feature(const_fn, raw))]
#![forbid(missing_docs)]

/*!
    # rel-ptr

    `rel-ptr` a library for relative pointers, which can be used to create
    moveable self-referential types. This library was inspired by
    Johnathan Blow's work on Jai, where he added relative pointers
    as a primitive into Jai.

    A relative pointer is a pointer that uses an offset and it's current location to
    calculate where it points to.

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
    # fn main() {
    # use rel_ptr::RelPtr;
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

            this.ptr.set(&this.value.0).unwrap();

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
    # }
    ```

    This example is contrived, and only useful as an example.
    In this example, we can see a few important parts to safe moveable self-referential types,
    lets walk through them.

    First, the definition of `SelfRef`, it contains a value and a relative pointer, the relative pointer that will point into the tuple inside of `SelfRef.value` to the `String`. There are no lifetimes involved because they would either make `SelfRef` immovable, or they could not be resolved correctly.

    We see a pattern inside of `SelfRef::new`, first create the object, and use the sentinel `RelPtr::null()` and immediately afterwards assigning it a value using `RelPtr::set` and unwraping the result. This unwrapping is get quick feedback on whether or not the pointer was set, if it wasn't set then we can increase the size of the offset and resolve that.

    Once the pointer is set, moving the struct is still safe because it is using a *relative* pointer, so it doesn't matter where it is, only it's offset from its pointee.
    In `SelfRef::fst` we use `RelPtr::as_ref_unchecked` because it is impossible to invalidate the pointer. It is impossible because we cannot
    set the relative pointer directly, and we cannot change the offsets of the fields of `SelfRef` after the relative pointer is set.
*/

#[cfg(feature = "no_std")]
extern crate core as std;

#[cfg(test)]
mod tests;

#[cfg(feature = "nightly")]
mod nightly;

mod traits;
mod error;

#[cfg(feature = "nightly")]
pub use self::nightly::*;
pub use self::traits::*;
pub use self::error::*;

use std::marker::PhantomData;
use std::ptr::NonNull;

macro_rules! impl_delta_zeroable {
    ($($type:ty),* $(,)?) => {$(
        unsafe impl Delta for $type {
            type Error = IntegerDeltaError;

            fn sub(a: *const u8, b: *const u8) -> Result<Self, Self::Error> {
                let del = match isize::checked_sub(a as usize as _, b as usize as _) {
                    Some(del) => del,
                    None => return Err(IntegerDeltaError(IntegerDeltaErrorImpl::Sub(a as usize, b as usize)))
                };

                if std::mem::size_of::<Self>() < std::mem::size_of::<isize>() && (
                    (Self::min_value() as isize) > del ||
                    (Self::max_value() as isize) < del
                )
                {
                    Err(IntegerDeltaError(IntegerDeltaErrorImpl::Conversion(del)))
                } else {
                    Ok(del as _)
                }
            }

            unsafe fn sub_unchecked(a: *const u8, b: *const u8) -> Self {
                use unreachable::UncheckedOptionExt;

                isize::checked_sub(a as usize as _, b as usize as _).unchecked_unwrap() as _
            }

            unsafe fn add(self, a: *const u8) -> *mut u8 {
                <*const u8>::offset(a, self as isize) as *mut u8
            }
        }

        impl Nullable for $type {
            const NULL: Self = 0;
        }
    )*};
}

impl_delta_zeroable! { i8, i16, i32, i64, i128, isize }

/**
 * This represents a relative pointers
 *
 * A relative pointer stores an offset, and uses its
 * that in combination with its current position in memory
 * to point to a value
 *
 * See crate documentation for more information
 * 
 * # Safety
 * 
 * When using `core::num::NonZero*`, it is UB to have the `RelPtr` point to itself, this could be achieved
 * with 
 * 
 * If you use `RelPtr::from(offset)`, then you must ensure that the relative pointer is set with the
 * given functions to avoid UB
*/
pub struct RelPtr<T: ?Sized + MetaData, I: Delta = isize>(I, T::Data, PhantomData<*mut T>);

// Ergonomics and ptr like impls

impl<T: ?Sized + MetaData, I: Delta> Copy for RelPtr<T, I> {}
impl<T: ?Sized + MetaData, I: Delta> Clone for RelPtr<T, I> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T: ?Sized + MetaData, I: Delta> Eq for RelPtr<T, I> {}
impl<T: ?Sized + MetaData, I: Delta> PartialEq for RelPtr<T, I> {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self, other)
    }
}

impl<T: ?Sized + MetaData, I: Delta> From<I> for RelPtr<T, I> {
    fn from(i: I) -> Self {
        Self(i, <T as MetaData>::Data::default(), PhantomData)
    }
}

// Core api

impl<T: ?Sized + MetaData, I: Nullable> RelPtr<T, I> {
    /**
     * A null relative pointer has an offset of 0, (points to itself)
     */
    #[inline(always)]
    pub fn null() -> Self {
        Self(I::NULL, <T as MetaData>::Data::default(), PhantomData)
    }

    /**
     * Check if relative pointer is null
     */
    #[inline(always)]
    pub fn is_null(&self) -> bool {
        self.0 == I::NULL
    }
}

impl<T: ?Sized + MetaData, I: Delta> RelPtr<T, I> {
    /**
     * set the offset of a relative pointer,
     * if the offset cannot be calculated using the given
     * `Delta`, then `Err` will be returned, and there will be
     * **no** change to the offset
     */
    #[inline]
    pub fn set(&mut self, value: &T) -> Result<(), I::Error> {
        let (ptr, meta) = T::decompose(value);

        self.0 = I::sub(ptr, self as *mut Self as _)?;
        self.1 = meta;

        Ok(())
    }

    /**
     * set the offset of a relative pointer,
     *
     * # Safety
     *
     * if the offset is out of bounds for the given `Delta`
     * then it's value is UB
     */
    #[inline]
    pub unsafe fn set_unchecked(&mut self, value: &T) {
        let (ptr, meta) = T::decompose(value);

        self.0 = I::sub_unchecked(ptr, self as *mut Self as _);
        self.1 = meta;
    }

    /**
     * Converts the relative pointer into a normal raw pointer
     *
     * # Safety
     *
     * You must ensure that the relative pointer was successfully set before
     * calling this function and that the value pointed to does not change it's
     * offset relative to `RelPtr`
     *
     * if `RelPtr::set` was never called successfully, this function is UB
     */
    #[inline]
    pub unsafe fn as_raw_unchecked(&self) -> *mut T {
        T::compose(self.0.add(self as *const Self as _) as _, self.1)
    }

    /**
     * Converts the relative pointer into a normal raw pointer
     *
     * # Safety
     *
     * Same as `RelPtr::as_raw_unchecked`
     */
    #[inline]
    pub unsafe fn as_non_null_unchecked(&self) -> NonNull<T> {
        NonNull::new_unchecked(self.as_raw_unchecked())
    }

    /**
     * Gets a reference from the relative pointer
     *
     * # Safety
     *
     * Same as `RelPtr::as_raw_unchecked`
     */
    #[inline]
    pub unsafe fn as_ref_unchecked(&self) -> &T {
        &*self.as_raw_unchecked()
    }

    /**
     * Gets a mutable reference from the relative pointer
     *
     * # Safety
     *
     * Same as `RelPtr::as_raw_unchecked`
     */
    #[inline]
    pub unsafe fn as_mut_unchecked(&mut self) -> &mut T {
        &mut *self.as_raw_unchecked()
    }
}

impl<T: ?Sized + MetaData, I: Nullable> RelPtr<T, I> {
    /**
     * Converts the relative pointer into a normal raw pointer
     *
     * Note: if `self.is_null()` then a null pointer will be returned
     *
     * # Safety
     *
     * You must ensure that if the relative pointer was successfully set then 
     * the value pointed to does not change it's offset relative to `RelPtr`
     *
     * if the relative pointer was not successfully set `RelPtr::as_raw` returns null,
     * this function is safe for all types except for trait objects
     * because the only way to construct a `RelPtr` is to make a null ptr and change it
     * through `RelPtr::set`, but with trait objects it is impossible to create a v-table
     * so it will have an invalid v-table (which is UB)
     */
    #[inline]
    pub unsafe fn as_raw(&self) -> *mut T {
        if self.is_null() {
            T::compose(std::ptr::null_mut(), T::Data::default())
        } else {
            self.as_raw_unchecked()
        }
    }

    /**
     * Converts the relative pointer into a normal raw pointer
     *
     * # Safety
     *
     * Same as `RelPtr::as_raw`
     */
    #[inline]
    pub unsafe fn as_non_null(&self) -> Option<NonNull<T>> {
        self.as_ref().map(NonNull::from)
    }

    /**
     * Gets a reference from the relative pointer,
     * if the relative pointer is null, then `None` is
     * returned
     *
     * # Safety
     *
     * Same as `RelPtr::as_raw`
     */
    #[inline]
    pub unsafe fn as_ref(&self) -> Option<&T> {
        <*const T>::as_ref(self.as_raw())
    }

    /**
     * Gets a mutable reference from the relative pointer,
     * if the relative pointer is null, then `None` is
     * returned
     *
     * # Safety
     *
     * Same as `RelPtr::as_raw`
     */
    #[inline]
    pub unsafe fn as_mut(&mut self) -> Option<&mut T> {
        <*mut T>::as_mut(self.as_raw())
    }
}
