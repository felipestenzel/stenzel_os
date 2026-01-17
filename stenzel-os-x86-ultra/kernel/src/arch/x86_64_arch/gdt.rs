//! GDT + TSS (inclui segmentos de usuário para ring3).
//!
//! Por quê isso importa?
//! - `iretq` para ring3 exige descritores de código/dados com DPL=3.
//! - Interrupções enquanto em ring3 exigem um RSP0 válido no TSS para trocar de stack.
//! - `syscall/sysret` usa seletores configurados no MSR IA32_STAR.

#![allow(dead_code)]

use core::cell::UnsafeCell;
use core::ptr::addr_of_mut;
use spin::Once;
use x86_64::instructions::segmentation::{Segment, CS, DS, ES, SS};
use x86_64::instructions::tables::load_tss;
use x86_64::structures::gdt::{Descriptor, GlobalDescriptorTable, SegmentSelector};
use x86_64::structures::tss::TaskStateSegment;
use x86_64::VirtAddr;

pub const DOUBLE_FAULT_IST_INDEX: u16 = 0;
pub const NMI_IST_INDEX: u16 = 1;

#[derive(Debug, Clone, Copy)]
pub struct Selectors {
    pub kernel_code: SegmentSelector,
    pub kernel_data: SegmentSelector,
    pub user_data: SegmentSelector,
    pub user_code: SegmentSelector,
    pub tss: SegmentSelector,
}

// Wrapper para permitir acesso seguro a statics mutáveis
#[repr(transparent)]
struct SyncUnsafeCell<T>(UnsafeCell<T>);
unsafe impl<T> Sync for SyncUnsafeCell<T> {}

impl<T> SyncUnsafeCell<T> {
    const fn new(value: T) -> Self {
        Self(UnsafeCell::new(value))
    }

    #[allow(clippy::mut_from_ref)]
    unsafe fn get_mut(&self) -> &mut T {
        &mut *self.0.get()
    }
}

// Statics para GDT/TSS - devem viver 'static
static TSS: SyncUnsafeCell<TaskStateSegment> = SyncUnsafeCell::new(TaskStateSegment::new());
static GDT: SyncUnsafeCell<GlobalDescriptorTable> = SyncUnsafeCell::new(GlobalDescriptorTable::new());
static SELECTORS: Once<Selectors> = Once::new();

// Stack IST dedicado para double fault (precisa ser static para lifetime)
static mut DF_STACK: [u8; 4096 * 5] = [0; 4096 * 5];
// Stack IST dedicado para NMI (precisa ser static para lifetime)
static mut NMI_STACK: [u8; 4096 * 5] = [0; 4096 * 5];

pub fn init() {
    unsafe {
        // Usa addr_of_mut! para evitar criar referência a static mut
        let df_stack_ptr = addr_of_mut!(DF_STACK);
        let df_stack_top = VirtAddr::from_ptr((df_stack_ptr as *const u8).add(4096 * 5));

        let nmi_stack_ptr = addr_of_mut!(NMI_STACK);
        let nmi_stack_top = VirtAddr::from_ptr((nmi_stack_ptr as *const u8).add(4096 * 5));

        let tss = TSS.get_mut();
        tss.interrupt_stack_table[DOUBLE_FAULT_IST_INDEX as usize] = df_stack_top;
        tss.interrupt_stack_table[NMI_IST_INDEX as usize] = nmi_stack_top;
        // RSP0 será configurado pelo scheduler conforme o processo em execução.
        tss.privilege_stack_table[0] = VirtAddr::new(0);

        let gdt = GDT.get_mut();
        let kernel_code = gdt.append(Descriptor::kernel_code_segment());
        let kernel_data = gdt.append(Descriptor::kernel_data_segment());
        let user_data = gdt.append(Descriptor::user_data_segment());
        let user_code = gdt.append(Descriptor::user_code_segment());
        let tss_sel = gdt.append(Descriptor::tss_segment(tss));

        let selectors = Selectors {
            kernel_code,
            kernel_data,
            user_data,
            user_code,
            tss: tss_sel,
        };

        gdt.load();

        CS::set_reg(selectors.kernel_code);
        DS::set_reg(selectors.kernel_data);
        ES::set_reg(selectors.kernel_data);
        SS::set_reg(selectors.kernel_data);
        load_tss(selectors.tss);

        SELECTORS.call_once(|| selectors);
    }
}

#[inline]
pub fn selectors() -> Selectors {
    *SELECTORS.get().expect("GDT não inicializada")
}

#[inline]
pub fn kernel_code_selector() -> SegmentSelector {
    selectors().kernel_code
}

#[inline]
pub fn kernel_data_selector() -> SegmentSelector {
    selectors().kernel_data
}

#[inline]
pub fn user_code_selector() -> SegmentSelector {
    selectors().user_code
}

#[inline]
pub fn user_data_selector() -> SegmentSelector {
    selectors().user_data
}

/// Atualiza o RSP0 no TSS (stack usada quando uma interrupção acontece em ring3).
pub fn set_kernel_stack_top(rsp0: u64) {
    unsafe {
        TSS.get_mut().privilege_stack_table[0] = VirtAddr::new(rsp0);
    }
}
