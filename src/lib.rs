//! Enable light references to trait
//!
//! # Intro
//!
//! ## What are trait object and virtual table ?
//!
//! In rust, you can have dynammic dispatch with the so-called Trait object.
//! Here is a typical example
//!
//! ```
//! trait Shape { fn area(&self) -> f32; }
//! struct Rectangle { w: f32, h : f32 }
//! impl Shape for Rectangle { fn area(&self) -> f32 { self.w * self.h } }
//! struct Circle { r: f32 }
//! impl Shape for Circle { fn area(&self) -> f32 { 3.14 * self.r * self.r } }
//!
//! // Given an array of Shape, compute the sum of their area
//! fn total_area(list: &[&dyn Shape]) -> f32 {
//!     list.iter().map(|x| x.area()).fold(0., |a, b| a+b)
//! }
//! ```
//! In this example the function `total_area` takes a reference of trait objects that implement
//! the `Shape` trait. Internally, this `&dyn Shape` reference is composed of two pointer:
//! a pointer to the object, and a pointer to a virtual table. The virtual table is a static
//! structure containing the function pointer to the `area` function. Such virtual table exist
//! for each type that implements the trait, but each instance of the same type share the same
//! virtual table. Having only a pointer to the struct itself would not be enough as the
//! `total_area` does not know the exact type of what it is pointed to, so it would not know from
//! ` with `impl` to call the `area`function.
//!
//! Other language such as C++ implements that differently: in C++, each instance of a dynamic class
//! has a pointer to the virtual table, inside of the class. So the pointer are just normal
//! pointers.
//!
//! Both approach have pros and cons: in Rust, the object themselves are a bit smaller as they
//! do not have a pointer to the virtual table. They can also implement trait from other crates
//! which would not work in C++ as it would have to somehow put the pointer to the virtual table
//! inside the object. But rust pointer to trait are twice as big as normal pointer. Which is
//! usually not a problem. Unless of course you want to pack many trait object reference in a vector
//! in constrained memory, or pass them through ffi to C function that only handle pointer as data.
//! That's where this crate comes in!
//!
//! ## Light references
//!
//! This crates allows to easily opt in to light references to trait for a type, by having
//! pointers to the virtual table within the object.
//!
//! ```rust
//! # use vptr::*;
//! trait Shape { fn area(&self) -> f32; }
//! #[vptr(Shape)]
//! struct Rectangle { w: f32, h : f32 }
//! impl Shape for Rectangle { fn area(&self) -> f32 { self.w * self.h } }
//! #[vptr(Shape)]
//! struct Circle { r: f32 }
//! impl Shape for Circle { fn area(&self) -> f32 { 3.14 * self.r * self.r } }
//!
//! // Given an array of Shape, compute the sum of their area
//! fn total_area(list: &[LightRef<Shape>]) -> f32 {
//!     list.iter().map(|x| x.area()).fold(0., |a, b| a+b)
//! }
//! ```
//!
//! Same as before, but we added `#[vptr(Shape)]` and are now using `LightRef<Shape>` instead of
//! `&dyn Shame`.  The difference is that the LightRef has only the size of one pointer
//!
//! # The `#[vptr]` macro
//!
//! The `#[vptr(Trait)]` macro can be applied to a struct and it adds members to the struct
//! with pointer to the vtable, these members are of type Vptr<S, Trait>, where S is the struct.
//! The macro also implements the `HasVPtr` trait which allow the creation of `LightRef` for this
//!
//! You probably want to derive from `Default`, otherwise, the extra fields needs to be initialized
//! manually (with `Default::default()` or `VPtr::new()`)
//!
//! ```rust
//! # use std::{mem, fmt::{self, Display}};
//! # use vptr::*;
//! trait Shape { fn area(&self) -> f32; }
//! #[vptr(Shape, ToString)] // There can be several traits
//! #[derive(Default)]
//! struct Rectangle { w: f32, h : f32 }
//!
//! // The traits within #[vptr(...)] need to be implemented for that type
//! impl Shape for Rectangle { fn area(&self) -> f32 { self.w * self.h } }
//! impl Display for Rectangle {
//!   fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
//!      write!(fmt, "Rectangle ({} x {})", self.w, self.h)
//!   }
//! }
//!
//! // [...]
//! let mut r1 = Rectangle::default();
//! r1.w = 10.; r1.h = 5.;
//! let ref1 = LightRef::<Shape>::from(&r1);
//! assert_eq!(mem::size_of::<LightRef<Shape>>(), mem::size_of::<usize>());
//! assert_eq!(ref1.area(), 50.);
//!
//! // When not initializing with default, you must initialize the vptr's manually
//! let r2 = Rectangle{ w: 1., h: 2., ..Default::default() };
//! let r3 = Rectangle{ w: 1., h: 2., vptr_Shape: VPtr::new(), vptr_ToString: VPtr::new() };
//!
//! // Also work with tuple struct
//! #[vptr(Shape)] struct Point(u32, u32);
//! impl Shape for Point { fn area(&self) -> f32 { 0. } }
//! let p = Point(1, 2, VPtr::new());
//! let pointref = LightRef::from(&p);
//! assert_eq!(pointref.area(), 0.);
//! ```

pub use ::vptr_macros::vptr;
use std::marker::PhantomData;

/// Represent a pointer to a virtual table to the trait `Trait` that is to be embedded in
/// a structure `T`
///
/// One should not need to use this structure directly, it is going to be created by the `vptr`
/// procedural macro.
#[derive(Clone, Copy, Eq, Hash, PartialEq, PartialOrd)]
pub struct VPtr<T, Trait : ?Sized> where T : HasVPtr<Trait> {
    vtable : &'static VTableData,
    phantom : PhantomData<(*const T, *const Trait)>
}

impl<T, Trait : ?Sized> VPtr<T, Trait> where T: HasVPtr<Trait> {
    // Creates a new VPtr initialized to a pointer of the vtable of the `Trait` for the type `T`.
    // Same as VPtr::default()
    pub fn new() -> Self {
        VPtr{vtable : T::init(),  phantom: PhantomData }
    }
}

impl<T, Trait : ?Sized> Default for VPtr<T, Trait> where T: HasVPtr<Trait> {
    // Creates a new VPtr initialized to a pointer of the vtable of the `Trait` for the type `T`.
    // Same as VPtr::new()
    fn default() -> Self {
        VPtr::new()
    }
}

impl<T, Trait : ?Sized> std::fmt::Debug for VPtr<T, Trait> where T: HasVPtr<Trait> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.pad("VPtr")
    }
}

/// This trait indicate that the type has a VPtr field to the trait `Trait`
///
/// You should not implement this trait yourself, it is implemented by the `vptr` macro
pub unsafe trait HasVPtr<Trait : ?Sized> {
    /// Initialize a VTableData suitable to initialize the VPtr within Self
    fn init() -> &'static VTableData;

    /// return the a reference of the VPtr within Self
    fn get_vptr(&self) -> &VPtr<Self, Trait> where Self: Sized;

    fn as_light_ref(&self) -> LightRef<Trait> where Self: Sized { LightRef::new(self.get_vptr()) }
}


/// A light reference (size = `size_of(usize)`) to an object implementing the trait `Trait`
#[derive(Copy, Clone, Eq, PartialEq)]
pub struct LightRef<'a, Trait : ?Sized> {
    ptr : &'a &'static VTableData,
    phantom : PhantomData<&'a Trait>
}

impl<'a, Trait : ?Sized> LightRef<'a, Trait> {
    pub fn new<T: HasVPtr<Trait>>(ptr : &'a VPtr<T, Trait>) -> Self {
        LightRef{ ptr: &ptr.vtable, phantom: PhantomData }
    }
}

impl<'a, Trait : ?Sized + 'a> ::core::ops::Deref for LightRef<'a, Trait> {
    type Target = Trait;

    fn deref(&self) -> &Self::Target {
        unsafe {
            let VTableData{offset, vtable} = **self.ptr;
            let p = (self.ptr as *const _ as *const u8).offset(-offset) as *const ();
            internal::TransmuterTO::<Trait>{ to: internal::TraitObject{ data: p, vtable: vtable } }.ptr
        }
    }
}

impl<'a, Trait : ?Sized + 'a> ::core::borrow::Borrow<Trait> for LightRef<'a, Trait> {
    fn borrow(&self) -> &Trait {
        &**self
    }
}

impl<'a, Trait : ?Sized + 'a, T: HasVPtr<Trait>> From<&'a T> for LightRef<'a, Trait> {
    fn from(f: &'a T) -> Self {
        LightRef::new(f.get_vptr())
    }
}

/// The data structure generated by the `#[vptr]` macro
///
/// You should normaly not use directly this struct
#[derive(Eq, Hash, PartialEq, PartialOrd)]
pub struct VTableData {
    /// Offset, in byte, of the VPtr field within the struct
    pub offset : isize,
    /// Pointer to the actual vtable generated by rust (i.e., the second pointer in a TraitObject,
    /// or core::raw::TraitObject::vtable)
    pub vtable : *const (),
}
unsafe impl std::marker::Sync for VTableData {}


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
    pub use super::*;

    mod vptr {
        pub use super::*;
    }



    trait MyTrait {
        fn myfn(&self)  -> u32;
    }

    #[vptr(MyTrait)]
    #[derive(Default)]
    struct Foobar2 {
        q: u32
    }

    impl MyTrait for Foobar2 {
        fn myfn(&self)  -> u32 {
            self.q + 4
        }
    }

    #[test]
    fn it_works2() {
        let mut f = Foobar2::default();
        f.q = 5;
        assert_eq!(f.myfn(), 9);

        let xx = f.as_light_ref();
        assert_eq!(xx.myfn(), 9);

    }




    #[vptr(MyTrait, SomeOtherTrait)]
    #[derive(Default, Debug)]
    struct Foobar3 {
        q: u32
    }

    impl MyTrait for Foobar3 {
        fn myfn(&self)  -> u32 {
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

        let xx : LightRef<MyTrait> = f.as_light_ref();
        assert_eq!(xx.myfn(), 9);

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

        let xx : LightRef<MyTrait> = f.as_light_ref();
        assert_eq!(xx.myfn(), 9);

    }
*/

    #[vptr(MyTrait)]
    #[derive(Default)]
    struct WithLifeTime<'a> {
        pub foo: Option<&'a u32>
    }

    impl<'a> MyTrait for WithLifeTime<'a> {
        fn myfn(&self)  -> u32 {
            *self.foo.unwrap_or(&0)
        }
    }

    #[test]
    fn with_lifetime() {
        let mut f = WithLifeTime::default();
        let x = 43;
        f.foo = Some(&x);
        assert_eq!(f.myfn(), 43);

        let xx : LightRef<MyTrait> = f.as_light_ref();
        assert_eq!(xx.myfn(), 43);

    }

    #[vptr(MyTrait)]
    struct Tuple(u32, u32);

    impl MyTrait for Tuple { fn myfn(&self) -> u32 { self.1 } }
    #[test]
    fn tuple() {
        let f = Tuple(42, 43, Default::default());
        assert_eq!(f.myfn(), 43);

        let xx : LightRef<_> = f.as_light_ref();
        assert_eq!(xx.myfn(), 43);

    }



    #[vptr(MyTrait)]
    struct Empty1;

    impl MyTrait for Empty1 { fn myfn(&self) -> u32 { 88 } }

    #[test]
    fn empty_struct() {
        let f = Empty1(VPtr::new());
        assert_eq!(f.myfn(), 88);

        let xx : LightRef<MyTrait> = f.as_light_ref();
        assert_eq!(xx.myfn(), 88);

    }



}
