/* Copyright (C) 2019 Olivier Goffart <ogoffart@woboq.com>

Permission is hereby granted, free of charge, to any person obtaining a copy of this software and
associated documentation files (the "Software"), to deal in the Software without restriction,
including without limitation the rights to use, copy, modify, merge, publish, distribute, sublicense,
and/or sell copies of the Software, and to permit persons to whom the Software is furnished to do so,
subject to the following conditions:

The above copyright notice and this permission notice shall be included in all copies or substantial
portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT
NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND
NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES
OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN
CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.
*/

/*! Enable thin references to trait

# Intro

## What are trait object and virtual table ?

In rust, you can have dynamic dispatch with the so-called Trait object.
Here is a typical example

```rust
trait Shape { fn area(&self) -> f32; }
struct Rectangle { w: f32, h : f32 }
impl Shape for Rectangle { fn area(&self) -> f32 { self.w * self.h } }
struct Circle { r: f32 }
impl Shape for Circle { fn area(&self) -> f32 { 3.14 * self.r * self.r } }

// Given an array of Shape, compute the sum of their area
fn total_area(list: &[&dyn Shape]) -> f32 {
    list.iter().map(|x| x.area()).fold(0., |a, b| a+b)
}
```
In this example the function `total_area` takes a reference of trait objects that implement
the `Shape` trait. Internally, this `&dyn Shape` reference is composed of two pointer:
a pointer to the object, and a pointer to a virtual table. The virtual table is a static
structure containing the function pointer to the `area` function. Such virtual table exist
for each type that implements the trait, but each instance of the same type share the same
virtual table. Having only a pointer to the struct itself would not be enough as the
`total_area` does not know the exact type of what it is pointed to, so it would not know from
which `impl` to call the `area` function.

This box diagram shows a simplified representation of the memory layout

```ascii
   &dyn Shape      ╭──────> Rectangle     ╭─> vtable of Shape for Rectangle
 ┏━━━━━━━━━━━━━┓   │       ┏━━━━━━━━━┓    │        ┏━━━━━━━━━┓
 ┃ data        ┠───╯       ┃ w       ┃    │        ┃ area()  ┃
 ┣━━━━━━━━━━━━━┫           ┣━━━━━━━━━┫    │        ┣━━━━━━━━━┫
 ┃ vtable ptr  ┠─────╮     ┃ h       ┃    │        ┃ drop()  ┃
 ┗━━━━━━━━━━━━━┛     │     ┗━━━━━━━━━┛    │        ┣━━━━━━━━━┫
                     ╰────────────────────╯        ┃ size    ┃
                                                   ╏         ╏
```

Other languages such as C++ implements that differently: in C++, each instance of a dynamic class
has a pointer to the virtual table, inside of the class. So just a normal pointer to the base class
is enough to do dynamic dispatch

Both approaches have pros and cons: in Rust, the object themselves are a bit smaller as they
do not have a pointer to the virtual table. They can also implement trait from other crates
which would not work in C++ as it would have to somehow put the pointer to the virtual table
inside the object. But rust pointer to trait are twice as big as normal pointer. Which is
usually not a problem. Unless of course you want to pack many trait object reference in a vector
in constrained memory, or pass them through ffi to C function that only handle pointer as data.
That's where this crate comes in!

## Thin references

This crates allows to easily opt in to thin references to trait for a type, by having
pointers to the virtual table within the object.

```rust
use vptr::vptr;
trait Shape { fn area(&self) -> f32; }
#[vptr(Shape)]
struct Rectangle { w: f32, h : f32 }
impl Shape for Rectangle { fn area(&self) -> f32 { self.w * self.h } }
#[vptr(Shape)]
struct Circle { r: f32 }
impl Shape for Circle { fn area(&self) -> f32 { 3.14 * self.r * self.r } }

// Given an array of Shape, compute the sum of their area
fn total_area(list: &[vptr::ThinRef<dyn Shape>]) -> f32 {
    list.iter().map(|x| x.area()).fold(0., |a, b| a+b)
}
```

Same as before, but we added `#[vptr(Shape)]` and are now using `ThinRef<Shape>` instead of
`&dyn Shame`.  The difference is that the ThinRef has only the size of one pointer


```ascii
 ThinRef<Shape>        Rectangle          ╭─>VTableData  ╭─>vtable of Shape for Rectangle
 ┏━━━━━━━━━━━━━┓      ┏━━━━━━━━━━━━┓ ╮    │  ┏━━━━━━━━┓  │     ┏━━━━━━━━━┓
 ┃ ptr         ┠──╮   ┃ w          ┃ │ ╭──│──┨ offset ┃  │     ┃ area()  ┃
 ┗━━━━━━━━━━━━━┛  │   ┣━━━━━━━━━━━━┫ ⎬─╯  │  ┣━━━━━━━━┫  │     ┣━━━━━━━━━┫
                  │   ┃ h          ┃ │    │  ┃ vtable ┠──╯     ┃ drop()  ┃
                  │   ┣━━━━━━━━━━━━┫ ╯    │  ┗━━━━━━━━┛        ┣━━━━━━━━━┫
                  ╰──>┃ vptr_Shape ┠──────╯                    ┃ size    ┃
                      ┗━━━━━━━━━━━━┛                           ╏         ╏
```


# The `#[vptr]` macro

The `#[vptr(Trait)]` macro can be applied to a struct and it adds members to the struct
with pointer to the vtable, these members are of type VPtr<S, Trait>, where S is the struct.
The macro also implements the `HasVPtr` trait which allow the creation of `ThinRef` for this

You probably want to derive from `Default`, otherwise, the extra fields needs to be initialized
manually (with `Default::default()` or `VPtr::new()`)

```rust
# use std::{mem, fmt::{self, Display}};
# use vptr::*;
trait Shape { fn area(&self) -> f32; }
#[vptr(Shape, ToString)] // There can be several traits
#[derive(Default)]
struct Rectangle { w: f32, h : f32 }

// The traits within #[vptr(...)] need to be implemented for that type
impl Shape for Rectangle { fn area(&self) -> f32 { self.w * self.h } }
impl Display for Rectangle {
  fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
     write!(fmt, "Rectangle ({} x {})", self.w, self.h)
  }
}

// [...]
let mut r1 = Rectangle::default();
r1.w = 10.; r1.h = 5.;
let ref1 = ThinRef::<dyn Shape>::from(&r1);
assert_eq!(mem::size_of::<ThinRef<dyn Shape>>(), mem::size_of::<usize>());
assert_eq!(ref1.area(), 50.);

// When not initializing with default, you must initialize the vptr's manually
let r2 = Rectangle{ w: 1., h: 2., ..Default::default() };
let r3 = Rectangle{ w: 1., h: 2., vptr_Shape: VPtr::new(), vptr_ToString: VPtr::new() };

// Also work with tuple struct
#[vptr(Shape)] struct Point(u32, u32);
impl Shape for Point { fn area(&self) -> f32 { 0. } }
let p = Point(1, 2, VPtr::new());
let pointref = ThinRef::from(&p);
assert_eq!(pointref.area(), 0.);

// The trait can be put in quote if it is too complex for a meta attribute
#[vptr("PartialEq<str>")]
#[derive(Default)]
struct MyString(String);
impl PartialEq<str> for MyString {
    fn eq(&self, other: &str) -> bool { self.0 == other }
}
let mystr = MyString("Hi".to_string(), VPtr::new());
let mystring_ref = ThinRef::from(&mystr);
assert!(*mystring_ref == *"Hi");
```
*/

#![cfg_attr(not(feature = "std"), no_std)]
#![warn(missing_docs)]
#[doc(inline)]
pub use ::vptr_macros::vptr;
use core::borrow::{Borrow, BorrowMut};
use core::marker::PhantomData;
use core::ops::{Deref, DerefMut};
use core::pin::Pin;
use core::ptr::NonNull;
#[cfg(feature = "std")]
use std::boxed::Box;

/// Represent a pointer to a virtual table to the trait `Trait` that is to be embedded in
/// a structure `T`
///
/// One should not need to use this structure directly, it is going to be created by the `vptr`
/// procedural macro.
#[derive(Clone, Copy, Eq, Hash, PartialEq, PartialOrd)]
pub struct VPtr<T, Trait: ?Sized>
where
    T: HasVPtr<Trait>,
{
    vtable: &'static VTableData,
    phantom: PhantomData<(*const T, *const Trait)>,
}

impl<T, Trait: ?Sized> VPtr<T, Trait>
where
    T: HasVPtr<Trait>,
{
    /// Creates a new VPtr initialized to a pointer of the vtable of the `Trait` for the type `T`.
    /// Same as VPtr::default()
    pub fn new() -> Self {
        VPtr {
            vtable: T::init(),
            phantom: PhantomData,
        }
    }
}

impl<T, Trait: ?Sized> Default for VPtr<T, Trait>
where
    T: HasVPtr<Trait>,
{
    // Creates a new VPtr initialized to a pointer of the vtable of the `Trait` for the type `T`.
    // Same as VPtr::new()
    fn default() -> Self {
        VPtr::new()
    }
}

#[cfg(feature = "std")]
impl<T, Trait: ?Sized> std::fmt::Debug for VPtr<T, Trait>
where
    T: HasVPtr<Trait>,
{
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.pad("VPtr")
    }
}

/// This trait indicate that the type has a VPtr field to the trait `Trait`
///
/// You should not implement this trait yourself, it is implemented by the `vptr` macro
///
/// Safety: For this to work correctly, the init() function must return a reference to a VTableData
/// with valid content (the offset and vtable pointer need to be correct for this type) and
/// get_vptr must return a reference of a field withi &self. The `#[vptr] macro does the right thing
pub unsafe trait HasVPtr<Trait: ?Sized> {
    /// Initialize a VTableData suitable to initialize the VPtr within Self
    fn init() -> &'static VTableData;

    /// return the a reference of the VPtr within Self
    fn get_vptr(&self) -> &VPtr<Self, Trait>
    where
        Self: Sized;

    /// return the a reference of the VPtr within Self
    fn get_vptr_mut(&mut self) -> &mut VPtr<Self, Trait>
    where
        Self: Sized;

    /// return a thin reference to self
    fn as_thin_ref(&self) -> ThinRef<Trait>
    where
        Self: Sized,
    {
        unsafe { ThinRef::new(self.get_vptr()) }
    }

    /// return a thin reference to self
    fn as_thin_ref_mut(&mut self) -> ThinRefMut<Trait>
    where
        Self: Sized,
    {
        unsafe { ThinRefMut::new(self.get_vptr_mut()) }
    }

    /// Map a pinned reference to to a pinned thin reference
    fn as_pin_thin_ref(self: Pin<&Self>) -> Pin<ThinRef<Trait>>
    where
        Self: Sized,
    {
        unsafe { Pin::new_unchecked(self.get_ref().as_thin_ref()) }
    }

    /// Map a pinned mutable reference to to a pinned mutable thin reference
    fn as_pin_thin_ref_mut(self: Pin<&mut Self>) -> Pin<ThinRefMut<Trait>>
    where
        Self: Sized,
    {
        unsafe { Pin::new_unchecked(self.get_unchecked_mut().as_thin_ref_mut()) }
    }
}

/// A thin reference (size = `size_of::<usize>()`) to an object implementing the trait `Trait`
///
/// This is like a reference to a trait (`&dyn Trait`) for struct that used
/// the macro `#[vptr(Trait)]`
///
/// See the crate documentation for example of usage.
///
/// The size is only the size of a single pointer:
/// ```rust
/// # use vptr::*;
/// # use std::mem;
/// # trait Trait { }
/// assert_eq!(mem::size_of::<ThinRef<dyn Trait>>(), mem::size_of::<usize>());
/// assert_eq!(mem::size_of::<Option<ThinRef<dyn Trait>>>(), mem::size_of::<usize>());
/// ```
pub struct ThinRef<'a, Trait: ?Sized> {
    ptr: &'a &'static VTableData,
    phantom: PhantomData<&'a Trait>,
}

impl<'a, Trait: ?Sized> ThinRef<'a, Trait> {
    /// Create a new reference from a reference to a VPtr
    ///
    /// Safety: the ptr must be a field of a reference to T
    unsafe fn new<T: HasVPtr<Trait>>(ptr: &'a VPtr<T, Trait>) -> Self {
        ThinRef {
            ptr: &ptr.vtable,
            phantom: PhantomData,
        }
    }
}

impl<'a, Trait: ?Sized + 'a> Deref for ThinRef<'a, Trait> {
    type Target = Trait;

    fn deref(&self) -> &Self::Target {
        unsafe {
            let VTableData { offset, vtable } = **self.ptr;
            let p = (self.ptr as *const _ as *const u8).offset(-offset) as *const ();
            internal::TransmuterTO::<Trait> {
                to: internal::TraitObject { data: p, vtable },
            }
            .ptr
        }
    }
}

impl<'a, Trait: ?Sized + 'a> Borrow<Trait> for ThinRef<'a, Trait> {
    fn borrow(&self) -> &Trait {
        &**self
    }
}

impl<'a, Trait: ?Sized + 'a, T: HasVPtr<Trait>> From<&'a T> for ThinRef<'a, Trait> {
    fn from(f: &'a T) -> Self {
        unsafe { ThinRef::new(f.get_vptr()) }
    }
}

// Cannot use #[derive()] because it gets the bounds wrong (See rust RFC #2353)
impl<'a, Trait: ?Sized> Clone for ThinRef<'a, Trait> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<'a, Trait: ?Sized> Copy for ThinRef<'a, Trait> {}

/// A thin reference (size = `size_of::<usize>()`) to an object implementing the trait `Trait`
///
/// Same as ThinRef but for mutable references
pub struct ThinRefMut<'a, Trait: ?Sized> {
    ptr: &'a mut &'static VTableData,
    phantom: PhantomData<&'a mut Trait>,
}

impl<'a, Trait: ?Sized> ThinRefMut<'a, Trait> {
    /// Create a new reference from a reference to a VPtr
    ///
    /// Safety: the ptr must be a field of a reference to T
    unsafe fn new<T: HasVPtr<Trait>>(ptr: &'a mut VPtr<T, Trait>) -> Self {
        ThinRefMut {
            ptr: &mut ptr.vtable,
            phantom: PhantomData,
        }
    }
}

impl<'a, Trait: ?Sized + 'a> Deref for ThinRefMut<'a, Trait> {
    type Target = Trait;

    fn deref(&self) -> &Self::Target {
        unsafe {
            let VTableData { offset, vtable } = **self.ptr;
            let p = (self.ptr as *const _ as *const u8).offset(-offset) as *const ();
            internal::TransmuterTO::<Trait> {
                to: internal::TraitObject { data: p, vtable },
            }
            .ptr
        }
    }
}

impl<'a, Trait: ?Sized + 'a> DerefMut for ThinRefMut<'a, Trait> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe {
            let VTableData { offset, vtable } = **self.ptr;
            let p = (self.ptr as *mut _ as *mut u8).offset(-offset) as *mut ();
            union Transmuter<T: ?Sized> {
                pub ptr: *mut T,
                pub to: internal::TraitObject,
            }
            let ptr = Transmuter::<Trait> {
                to: internal::TraitObject { data: p, vtable },
            }
            .ptr;
            &mut *ptr
        }
    }
}

impl<'a, Trait: ?Sized + 'a> Borrow<Trait> for ThinRefMut<'a, Trait> {
    fn borrow(&self) -> &Trait {
        &**self
    }
}

impl<'a, Trait: ?Sized + 'a> BorrowMut<Trait> for ThinRefMut<'a, Trait> {
    fn borrow_mut(&mut self) -> &mut Trait {
        &mut **self
    }
}

impl<'a, Trait: ?Sized + 'a, T: HasVPtr<Trait>> From<&'a mut T> for ThinRefMut<'a, Trait> {
    fn from(f: &'a mut T) -> Self {
        unsafe { ThinRefMut::new(f.get_vptr_mut()) }
    }
}

/// A Box of a trait with a size of `size_of::<usize>`
///
/// The ThinBox can be created from a Box which implement the HasVPtr<Trait>
///
///
/// ```rust
/// # use vptr::*;
/// trait Shape { fn area(&self) -> f32; }
/// #[vptr(Shape)]
/// #[derive(Default)]
/// struct Rectangle { w: f32, h : f32 }
/// impl Shape for Rectangle { fn area(&self) -> f32 { self.w * self.h } }
///
/// let r = Box::new(Rectangle { w: 5., h: 10., ..Default::default() });
/// let thin = ThinBox::from_box(r);
/// assert_eq!(thin.area(), 50.);
/// ```
///
/// The size is the size of a pointer
/// ```rust
/// # use vptr::*;
/// # use std::mem;
/// # trait Trait { }
/// assert_eq!(mem::size_of::<ThinBox<dyn Trait>>(), mem::size_of::<usize>());
/// assert_eq!(mem::size_of::<Option<ThinBox<dyn Trait>>>(), mem::size_of::<usize>());
/// ```
#[cfg(feature = "std")]
#[repr(transparent)]
pub struct ThinBox<Trait: ?Sized + 'static>(NonNull<&'static VTableData>, PhantomData<*mut Trait>);

#[cfg(feature = "std")]
#[allow(clippy::wrong_self_convention)]
impl<Trait: ?Sized + 'static> ThinBox<Trait> {
    /// Creates a ThinBox from a Box
    pub fn from_box<T: HasVPtr<Trait>>(f: Box<T>) -> Self {
        ThinBox(
            NonNull::from(&mut Box::leak(f).get_vptr_mut().vtable),
            PhantomData,
        )
    }
    /// Conver the ThinBox into a Box
    pub fn into_box(mut b: ThinBox<Trait>) -> Box<Trait> {
        let ptr = (&mut *ThinBox::as_thin_ref_mut(&mut b)) as *mut Trait;
        core::mem::forget(b);
        unsafe { Box::from_raw(ptr) }
    }

    /// As a ThinRef
    pub fn as_thin_ref(b: &ThinBox<Trait>) -> ThinRef<Trait> {
        ThinRef {
            ptr: unsafe { b.0.as_ref() },
            phantom: PhantomData,
        }
    }

    /// As a ThinRefMut
    pub fn as_thin_ref_mut(b: &mut ThinBox<Trait>) -> ThinRefMut<Trait> {
        ThinRefMut {
            ptr: unsafe { b.0.as_mut() },
            phantom: PhantomData,
        }
    }
}

#[cfg(feature = "std")]
impl<Trait: ?Sized + 'static> Drop for ThinBox<Trait> {
    fn drop(&mut self) {
        let ptr = &mut *ThinBox::as_thin_ref_mut(self) as *mut Trait;
        unsafe { Box::from_raw(ptr) };
    }
}

#[cfg(feature = "std")]
impl<Trait: ?Sized + 'static> Deref for ThinBox<Trait> {
    type Target = Trait;

    fn deref(&self) -> &Self::Target {
        let ptr = &*ThinBox::as_thin_ref(self) as *const Trait;
        unsafe { &*ptr }
    }
}

#[cfg(feature = "std")]
impl<Trait: ?Sized + 'static> DerefMut for ThinBox<Trait> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        let ptr = &mut *ThinBox::as_thin_ref_mut(self) as *mut Trait;
        unsafe { &mut *ptr }
    }
}

/// The data structure generated by the `#[vptr]` macro
///
/// You should normaly not use directly this struct
#[derive(Eq, Hash, PartialEq, PartialOrd)]
pub struct VTableData {
    /// Offset, in byte, of the VPtr field within the struct
    pub offset: isize,
    /// Pointer to the actual vtable generated by rust (i.e., the second pointer in a TraitObject,
    /// or core::raw::TraitObject::vtable)
    pub vtable: *const (),
}
unsafe impl core::marker::Sync for VTableData {}

/// A convenience module import the most important items
///
/// ```
/// use vptr::prelude::*;
/// ```
pub mod prelude {
    #[doc(no_inline)]
    pub use crate::{vptr, HasVPtr};
}

#[doc(hidden)]
pub mod internal {
    /// Internal struct used by the macro generated code
    /// Copy of core::raw::TraitObject since it is unstable
    #[doc(hidden)]
    #[repr(C)]
    #[derive(Copy, Clone)]
    pub struct TraitObject {
        pub data: *const (),
        pub vtable: *const (),
    }

    /// Internal struct used by the macro generated code
    #[doc(hidden)]
    pub union TransmuterPtr<T: 'static> {
        pub ptr: &'static T,
        pub int: isize,
    }

    /// Internal struct used by the macro generated code
    #[doc(hidden)]
    pub union TransmuterTO<'a, T: ?Sized + 'a> {
        pub ptr: &'a T,
        pub to: TraitObject,
    }
}

#[cfg(test)]
mod tests {
    pub use crate::{vptr, HasVPtr, ThinBox, ThinRef, ThinRefMut, VPtr};

    mod vptr {
        // Because otherwise, the generated code cannot access the vptr crate.
        pub use crate::*;
    }

    trait MyTrait {
        fn myfn(&self) -> u32;
    }

    #[vptr(MyTrait)]
    #[derive(Default)]
    struct Foobar2 {
        q: u32,
    }

    impl MyTrait for Foobar2 {
        fn myfn(&self) -> u32 {
            self.q + 4
        }
    }

    #[test]
    fn it_works2() {
        let mut f = Foobar2::default();
        f.q = 5;
        assert_eq!(f.myfn(), 9);

        let xx = f.as_thin_ref();
        assert_eq!(xx.myfn(), 9);
    }

    #[vptr(MyTrait, SomeOtherTrait)]
    #[derive(Default, Debug)]
    struct Foobar3 {
        q: u32,
    }

    impl MyTrait for Foobar3 {
        fn myfn(&self) -> u32 {
            self.q + 4
        }
    }

    trait SomeOtherTrait {}
    impl SomeOtherTrait for Foobar3 {}

    #[test]
    fn it_works3() {
        let mut f = Foobar3::default();
        f.q = 5;
        println!("{:?}", f);
        assert_eq!(f.myfn(), 9);

        {
            let xx: ThinRef<dyn MyTrait> = f.as_thin_ref();
            assert_eq!(xx.myfn(), 9);
        }

        {
            let xx: ThinRefMut<dyn MyTrait> = f.as_thin_ref_mut();
            assert_eq!(xx.myfn(), 9);
        }
    }

    /*
    #[vptr(MyTrait)]
    #[derive(Default)]
    struct WithGenerics<T> {
        pub foo: Vec<T>
    }

    impl<T> MyTrait for WithGenerics<T> {
        fn myfn(&self)  -> u32 {
            self.foo.len() as u32
        }
    }

    #[test]
    fn with_generics() {
        let mut f = WithGenerics::<u32>::default();
        f.foo.push(3);
        assert_eq!(f.myfn(), 1);

        let xx : ThinRef<dyn MyTrait> = f.as_thin_ref();
        assert_eq!(xx.myfn(), 9);

    }
    */

    #[vptr(MyTrait)]
    #[derive(Default)]
    struct WithLifeTime<'a> {
        pub foo: Option<&'a u32>,
    }

    impl<'a> MyTrait for WithLifeTime<'a> {
        fn myfn(&self) -> u32 {
            *self.foo.unwrap_or(&0)
        }
    }

    #[test]
    fn with_lifetime() {
        let mut f = WithLifeTime::default();
        let x = 43;
        f.foo = Some(&x);
        assert_eq!(f.myfn(), 43);

        let xx: ThinRef<dyn MyTrait> = f.as_thin_ref();
        assert_eq!(xx.myfn(), 43);
    }

    #[vptr(MyTrait)]
    struct Tuple(u32, u32);

    impl MyTrait for Tuple {
        fn myfn(&self) -> u32 {
            self.1
        }
    }
    #[test]
    fn tuple() {
        let f = Tuple(42, 43, Default::default());
        assert_eq!(f.myfn(), 43);

        let xx: ThinRef<_> = f.as_thin_ref();
        assert_eq!(xx.myfn(), 43);
    }

    #[vptr(MyTrait)]
    struct Empty1;

    impl MyTrait for Empty1 {
        fn myfn(&self) -> u32 {
            88
        }
    }

    #[test]
    fn empty_struct() {
        let f = Empty1(VPtr::new());
        assert_eq!(f.myfn(), 88);

        let xx: ThinRef<dyn MyTrait> = f.as_thin_ref();
        assert_eq!(xx.myfn(), 88);
    }

    #[vptr(std::fmt::Display)]
    struct TestDisplay {
        str: String,
    }
    impl std::fmt::Display for TestDisplay {
        fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(fmt, "Test {}", self.str)
        }
    }

    #[test]
    fn with_path() {
        let x = TestDisplay {
            str: "Hello".to_string(),
            vptr_Display: Default::default(),
        };
        let xx: ThinRef<dyn std::fmt::Display> = x.as_thin_ref();
        assert_eq!(xx.to_string(), "Test Hello");
    }

    #[test]
    fn test_trait_with_gen() {
        trait TraitWithGen<T> {
            fn compute(&self, x: T) -> u32;
        }
        #[vptr("TraitWithGen<u64>")]
        struct TestTraitWithGen {
            value: u32,
        }
        impl TraitWithGen<u64> for TestTraitWithGen {
            fn compute(&self, x: u64) -> u32 {
                self.value + (x as u32)
            }
        }

        let x = TestTraitWithGen {
            value: 44,
            vptr_TraitWithGen: Default::default(),
        };
        let xx: ThinRef<dyn TraitWithGen<u64>> = x.as_thin_ref();
        assert_eq!(core::mem::size_of_val(&xx), core::mem::size_of::<usize>());
        assert_eq!(xx.compute(66u64), 44 + 66);
    }

    #[test]
    fn copy() {
        let f = Tuple(2, 3, Default::default());
        let xx: ThinRef<_> = f.as_thin_ref();
        let xx2 = xx;
        assert_eq!(xx.myfn(), 3);
        assert_eq!(xx2.myfn(), 3);
    }

    #[test]
    fn pin() {
        use core::pin::Pin;
        {
            let mut f = Foobar3::default();
            f.q = 5;
            let f: Pin<&Foobar3> = unsafe { Pin::new_unchecked(&f) };
            let xx: Pin<ThinRef<dyn MyTrait>> = f.as_pin_thin_ref();
            assert_eq!(xx.myfn(), 9);
        }

        {
            let mut f = Foobar3::default();
            f.q = 8;
            let f: Pin<&mut Foobar3> = unsafe { Pin::new_unchecked(&mut f) };
            let xx: Pin<ThinRefMut<dyn MyTrait>> = f.as_pin_thin_ref_mut();
            assert_eq!(xx.myfn(), 12);
        }
    }

}
