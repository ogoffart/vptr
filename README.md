# vptr

Enable light references to trait

## Intro

### What are trait object and virtual table ?

In rust, you can have dynammic dispatch with the so-called Trait object.
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

This box diagram show a simplified representation of the memory layout

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

Other language such as C++ implements that differently: in C++, each instance of a dynamic class
has a pointer to the virtual table, inside of the class. So the pointer are just normal
pointers.

Both approach have pros and cons: in Rust, the object themselves are a bit smaller as they
do not have a pointer to the virtual table. They can also implement trait from other crates
which would not work in C++ as it would have to somehow put the pointer to the virtual table
inside the object. But rust pointer to trait are twice as big as normal pointer. Which is
usually not a problem. Unless of course you want to pack many trait object reference in a vector
in constrained memory, or pass them through ffi to C function that only handle pointer as data.
That's where this crate comes in!

### Light references

This crates allows to easily opt in to light references to trait for a type, by having
pointers to the virtual table within the object.

```rust
trait Shape { fn area(&self) -> f32; }
#[vptr(Shape)]
struct Rectangle { w: f32, h : f32 }
impl Shape for Rectangle { fn area(&self) -> f32 { self.w * self.h } }
#[vptr(Shape)]
struct Circle { r: f32 }
impl Shape for Circle { fn area(&self) -> f32 { 3.14 * self.r * self.r } }

// Given an array of Shape, compute the sum of their area
fn total_area(list: &[LightRef<Shape>]) -> f32 {
    list.iter().map(|x| x.area()).fold(0., |a, b| a+b)
}
```

Same as before, but we added `#[vptr(Shape)]` and are now using `LightRef<Shape>` instead of
`&dyn Shame`.  The difference is that the LightRef has only the size of one pointer


```ascii
 LightRef<Shape>       Rectangle          ╭─>VTableData  ╭─>vtable of Shape for Rectangle
 ┏━━━━━━━━━━━━━┓      ┏━━━━━━━━━━━━┓ ╮    │  ┏━━━━━━━━┓  │     ┏━━━━━━━━━┓
 ┃ ptr         ┠──╮   ┃ w          ┃ │ ╭──│──┨ offset ┃  │     ┃ area()  ┃
 ┗━━━━━━━━━━━━━┛  │   ┣━━━━━━━━━━━━┫ ⎬─╯  │  ┣━━━━━━━━┫  │     ┣━━━━━━━━━┫
                  │   ┃ h          ┃ │    │  ┃ vtable ┠──╯     ┃ drop()  ┃
                  │   ┣━━━━━━━━━━━━┫ ╯    │  ┗━━━━━━━━┛        ┣━━━━━━━━━┫
                  ╰──>┃ vptr_Shape ┠──────╯                    ┃ size    ┃
                      ┗━━━━━━━━━━━━┛                           ╏         ╏
```


## The `#[vptr]` macro

The `#[vptr(Trait)]` macro can be applied to a struct and it adds members to the struct
with pointer to the vtable, these members are of type VPtr<S, Trait>, where S is the struct.
The macro also implements the `HasVPtr` trait which allow the creation of `LightRef` for this

You probably want to derive from `Default`, otherwise, the extra fields needs to be initialized
manually (with `Default::default()` or `VPtr::new()`)

```rust
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
let ref1 = LightRef::<Shape>::from(&r1);
assert_eq!(mem::size_of::<LightRef<Shape>>(), mem::size_of::<usize>());
assert_eq!(ref1.area(), 50.);

// When not initializing with default, you must initialize the vptr's manually
let r2 = Rectangle{ w: 1., h: 2., ..Default::default() };
let r3 = Rectangle{ w: 1., h: 2., vptr_Shape: VPtr::new(), vptr_ToString: VPtr::new() };

// Also work with tuple struct
#[vptr(Shape)] struct Point(u32, u32);
impl Shape for Point { fn area(&self) -> f32 { 0. } }
let p = Point(1, 2, VPtr::new());
let pointref = LightRef::from(&p);
assert_eq!(pointref.area(), 0.);
```
