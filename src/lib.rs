
pub struct VTable<T, Trait : ?Sized> where T : InitVTable<T, Trait> {
    pub vtable : &'static VTableData,
    _phantom : ::std::marker::PhantomData<(*const T, *const Trait)>
}

impl<T,  Trait : ?Sized> Default for VTable<T, Trait> where T: InitVTable<T, Trait> {
    fn default() -> Self {
        VTable{vtable : T::init() ,   _phantom: Default::default() }
    }
}

pub struct LightRef<'a, Trait : ?Sized> {
    ptr : &'a &'static VTableData,
    _phantom : ::std::marker::PhantomData<&'a Trait>
}

impl<'a, Trait : ?Sized> LightRef<'a, Trait> {
    pub unsafe fn from(ptr : &'a &'static VTableData) -> Self {
        LightRef{ptr, _phantom: Default::default() }
    }
}

impl<'a, Trait : ?Sized + 'a> ::std::ops::Deref for LightRef<'a, Trait> {
    type Target = Trait;

    fn deref(&self) -> &Self::Target {
        unsafe {
            let VTableData{offset, vtable} = **self.ptr;
            let p = (self.ptr as *const _ as *const u8).offset(-offset) as *const ();
            TransmuterTO::<Trait>{ to: TraitObject{ data: p, vtable: vtable } }.ptr
        }
    }
}

pub trait InitVTable<T, Trait : ?Sized> {
    fn init() -> &'static VTableData;
    fn as_light_ref(&self) -> LightRef<Trait>;
}

pub struct VTableData {
    pub offset : isize,
    pub vtable : *const (),
}
unsafe impl std::marker::Sync for VTableData {}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct TraitObject {
    pub data: *const (),
    pub vtable: *const (),
}

#[doc(hidden)]
pub union TransmuterPtr<T: 'static> {
    pub ptr: &'static T,
    pub int: usize,
}

#[doc(hidden)]
pub union TransmuterTO<'a, T: ?Sized + 'a> {
    pub ptr: &'a T,
    pub to: TraitObject,
}


#[cfg(test)]
mod tests {
    use super::*;

    trait MyTrait {
        fn myfn(&self)  -> u32;
    }

    #[derive(Default)]
    #[repr(C)]
    struct Foobar {
        q: u32,
        v: VTable<Foobar, MyTrait>
    }

    impl MyTrait for Foobar {
        fn myfn(&self)  -> u32 {
            dbg!(self.q);
            dbg!(self as *const Self);
            self.q + 4
        }
    }


    impl InitVTable<Foobar, MyTrait> for Foobar {
        fn init() -> &'static VTableData {
            static VTABLE_FOR_Foobar_MyTrait : VTableData = VTableData{
                offset: memoffset::offset_of!(Foobar, v) as isize,
                vtable: unsafe {
                    let x: &'static Foobar = TransmuterPtr::<Foobar> { int: 0 }.ptr;
                    TransmuterTO::<MyTrait>{ ptr: x }.to.vtable
                }
            };
            &VTABLE_FOR_Foobar_MyTrait
        }

        fn as_light_ref<'a>(&'a self) -> LightRef<'a, MyTrait> {
            unsafe { LightRef::from(&self.v.vtable) }
        }
    }

    #[test]
    fn it_works() {
        let mut f = Foobar::default();
        f.q = 5;
        assert_eq!(f.myfn(), 9);

        let xx = f.as_light_ref();
        assert_eq!(xx.myfn(), 9);

    }
}
