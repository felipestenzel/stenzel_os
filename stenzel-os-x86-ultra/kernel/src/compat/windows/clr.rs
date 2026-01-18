//! .NET Common Language Runtime (CLR) Compatibility Layer
//!
//! Basic implementation of .NET Framework/CLR for Windows compatibility.
//! This provides:
//! - PE/.NET assembly parsing (CLI header, metadata)
//! - Basic IL (Intermediate Language) interpretation
//! - Type system fundamentals
//! - Basic BCL stubs (System namespace)
//!
//! Based on ECMA-335 (CLI) specification.

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU32, Ordering};

/// CLR result type
pub type ClrResult<T> = Result<T, ClrError>;

/// CLR error types
#[derive(Debug, Clone)]
pub enum ClrError {
    InvalidAssembly,
    InvalidMetadata,
    TypeNotFound(String),
    MethodNotFound(String),
    FieldNotFound(String),
    InvalidIL,
    StackOverflow,
    NullReference,
    InvalidCast,
    IndexOutOfRange,
    DivideByZero,
    OutOfMemory,
    SecurityException,
    FileNotFound(String),
    NotImplemented(String),
}

/// Object handle
pub type ObjectHandle = u64;

/// Type token
pub type TypeToken = u32;

/// Method token
pub type MethodToken = u32;

/// Field token
pub type FieldToken = u32;

/// String token
pub type StringToken = u32;

/// Global handle counter
static NEXT_HANDLE: AtomicU32 = AtomicU32::new(1);

fn next_handle() -> ObjectHandle {
    NEXT_HANDLE.fetch_add(1, Ordering::Relaxed) as u64
}

/// CLI header magic
const CLI_HEADER_MAGIC: u32 = 0x424A5342; // 'BSJB'

/// Metadata stream names
pub mod stream_names {
    pub const STRINGS: &str = "#Strings";
    pub const US: &str = "#US";
    pub const BLOB: &str = "#Blob";
    pub const GUID: &str = "#GUID";
    pub const TILDE: &str = "#~";
    pub const MINUS: &str = "#-";
}

/// Metadata table types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum MetadataTable {
    Module = 0x00,
    TypeRef = 0x01,
    TypeDef = 0x02,
    Field = 0x04,
    MethodDef = 0x06,
    Param = 0x08,
    InterfaceImpl = 0x09,
    MemberRef = 0x0A,
    Constant = 0x0B,
    CustomAttribute = 0x0C,
    FieldMarshal = 0x0D,
    DeclSecurity = 0x0E,
    ClassLayout = 0x0F,
    FieldLayout = 0x10,
    StandAloneSig = 0x11,
    EventMap = 0x12,
    Event = 0x14,
    PropertyMap = 0x15,
    Property = 0x17,
    MethodSemantics = 0x18,
    MethodImpl = 0x19,
    ModuleRef = 0x1A,
    TypeSpec = 0x1B,
    ImplMap = 0x1C,
    FieldRva = 0x1D,
    Assembly = 0x20,
    AssemblyProcessor = 0x21,
    AssemblyOs = 0x22,
    AssemblyRef = 0x23,
    AssemblyRefProcessor = 0x24,
    AssemblyRefOs = 0x25,
    File = 0x26,
    ExportedType = 0x27,
    ManifestResource = 0x28,
    NestedClass = 0x29,
    GenericParam = 0x2A,
    MethodSpec = 0x2B,
    GenericParamConstraint = 0x2C,
}

/// Element types (for signatures)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ElementType {
    End = 0x00,
    Void = 0x01,
    Boolean = 0x02,
    Char = 0x03,
    I1 = 0x04,
    U1 = 0x05,
    I2 = 0x06,
    U2 = 0x07,
    I4 = 0x08,
    U4 = 0x09,
    I8 = 0x0A,
    U8 = 0x0B,
    R4 = 0x0C,
    R8 = 0x0D,
    String = 0x0E,
    Ptr = 0x0F,
    ByRef = 0x10,
    ValueType = 0x11,
    Class = 0x12,
    Var = 0x13,
    Array = 0x14,
    GenericInst = 0x15,
    TypedByRef = 0x16,
    IntPtr = 0x18,
    UIntPtr = 0x19,
    FnPtr = 0x1B,
    Object = 0x1C,
    SzArray = 0x1D,
    MVar = 0x1E,
    CModReqd = 0x1F,
    CModOpt = 0x20,
    Internal = 0x21,
    Modifier = 0x40,
    Sentinel = 0x41,
    Pinned = 0x45,
}

impl ElementType {
    pub fn from_u8(val: u8) -> Option<Self> {
        match val {
            0x00 => Some(ElementType::End),
            0x01 => Some(ElementType::Void),
            0x02 => Some(ElementType::Boolean),
            0x03 => Some(ElementType::Char),
            0x04 => Some(ElementType::I1),
            0x05 => Some(ElementType::U1),
            0x06 => Some(ElementType::I2),
            0x07 => Some(ElementType::U2),
            0x08 => Some(ElementType::I4),
            0x09 => Some(ElementType::U4),
            0x0A => Some(ElementType::I8),
            0x0B => Some(ElementType::U8),
            0x0C => Some(ElementType::R4),
            0x0D => Some(ElementType::R8),
            0x0E => Some(ElementType::String),
            0x0F => Some(ElementType::Ptr),
            0x10 => Some(ElementType::ByRef),
            0x11 => Some(ElementType::ValueType),
            0x12 => Some(ElementType::Class),
            0x14 => Some(ElementType::Array),
            0x18 => Some(ElementType::IntPtr),
            0x19 => Some(ElementType::UIntPtr),
            0x1C => Some(ElementType::Object),
            0x1D => Some(ElementType::SzArray),
            _ => None,
        }
    }

    pub fn is_primitive(&self) -> bool {
        matches!(
            self,
            ElementType::Boolean
                | ElementType::Char
                | ElementType::I1
                | ElementType::U1
                | ElementType::I2
                | ElementType::U2
                | ElementType::I4
                | ElementType::U4
                | ElementType::I8
                | ElementType::U8
                | ElementType::R4
                | ElementType::R8
                | ElementType::IntPtr
                | ElementType::UIntPtr
        )
    }

    pub fn size(&self) -> usize {
        match self {
            ElementType::Void => 0,
            ElementType::Boolean | ElementType::I1 | ElementType::U1 => 1,
            ElementType::Char | ElementType::I2 | ElementType::U2 => 2,
            ElementType::I4 | ElementType::U4 | ElementType::R4 => 4,
            ElementType::I8 | ElementType::U8 | ElementType::R8 => 8,
            ElementType::IntPtr | ElementType::UIntPtr => 8, // 64-bit
            ElementType::Object | ElementType::String | ElementType::Class => 8, // Reference
            _ => 8, // Default to pointer size
        }
    }
}

/// IL opcode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ILOpcode {
    // Basic
    Nop,
    Break,
    Ldarg(u16),
    Ldarga(u16),
    Starg(u16),
    Ldloc(u16),
    Ldloca(u16),
    Stloc(u16),
    Ldnull,
    LdcI4(i32),
    LdcI8(i64),
    LdcR4(u32), // IEEE 754 bits
    LdcR8(u64), // IEEE 754 bits
    Dup,
    Pop,
    Jmp(MethodToken),
    Call(MethodToken),
    Calli,
    Ret,
    // Branch
    BrS(i8),
    BrfalseS(i8),
    BrtrueS(i8),
    BeqS(i8),
    BgeS(i8),
    BgtS(i8),
    BleS(i8),
    BltS(i8),
    BneUnS(i8),
    BgeUnS(i8),
    BgtUnS(i8),
    BleUnS(i8),
    BltUnS(i8),
    Br(i32),
    Brfalse(i32),
    Brtrue(i32),
    Beq(i32),
    Bge(i32),
    Bgt(i32),
    Ble(i32),
    Blt(i32),
    BneUn(i32),
    BgeUn(i32),
    BgtUn(i32),
    BleUn(i32),
    BltUn(i32),
    Switch(u32), // Count, followed by offsets
    // Indirect load
    LdindI1,
    LdindU1,
    LdindI2,
    LdindU2,
    LdindI4,
    LdindU4,
    LdindI8,
    LdindI,
    LdindR4,
    LdindR8,
    LdindRef,
    StindRef,
    StindI1,
    StindI2,
    StindI4,
    StindI8,
    StindR4,
    StindR8,
    // Arithmetic
    Add,
    Sub,
    Mul,
    Div,
    DivUn,
    Rem,
    RemUn,
    And,
    Or,
    Xor,
    Shl,
    Shr,
    ShrUn,
    Neg,
    Not,
    // Conversion
    ConvI1,
    ConvI2,
    ConvI4,
    ConvI8,
    ConvR4,
    ConvR8,
    ConvU4,
    ConvU8,
    ConvRUn,
    ConvOvfI1Un,
    ConvOvfI2Un,
    ConvOvfI4Un,
    ConvOvfI8Un,
    ConvOvfU1Un,
    ConvOvfU2Un,
    ConvOvfU4Un,
    ConvOvfU8Un,
    ConvOvfIUn,
    ConvOvfUUn,
    ConvU2,
    ConvU1,
    ConvI,
    ConvOvfI,
    ConvOvfU,
    ConvU,
    // Object model
    Callvirt(MethodToken),
    Cpobj(TypeToken),
    Ldobj(TypeToken),
    Ldstr(StringToken),
    Newobj(MethodToken),
    Castclass(TypeToken),
    Isinst(TypeToken),
    Unbox(TypeToken),
    Throw,
    Ldfld(FieldToken),
    Ldflda(FieldToken),
    Stfld(FieldToken),
    Ldsfld(FieldToken),
    Ldsflda(FieldToken),
    Stsfld(FieldToken),
    Stobj(TypeToken),
    Box(TypeToken),
    Newarr(TypeToken),
    Ldlen,
    Ldelema(TypeToken),
    LdelemI1,
    LdelemU1,
    LdelemI2,
    LdelemU2,
    LdelemI4,
    LdelemU4,
    LdelemI8,
    LdelemI,
    LdelemR4,
    LdelemR8,
    LdelemRef,
    StelemI,
    StelemI1,
    StelemI2,
    StelemI4,
    StelemI8,
    StelemR4,
    StelemR8,
    StelemRef,
    Ldelem(TypeToken),
    Stelem(TypeToken),
    UnboxAny(TypeToken),
    // Comparison
    Ceq,
    Cgt,
    CgtUn,
    Clt,
    CltUn,
    // Prefix
    Tail,
    Volatile,
    Unaligned(u8),
    Constrained(TypeToken),
    Readonly,
    // Extended
    Initobj(TypeToken),
    Cpblk,
    Initblk,
    Rethrow,
    Sizeof(TypeToken),
    Ldtoken(u32),
    Ldftn(MethodToken),
    Ldvirtftn(MethodToken),
    Localloc,
    Endfilter,
    Endfinally,
    Leave(i32),
    LeaveS(i8),
    // Invalid/unknown
    Invalid,
}

/// IL instruction with offset
#[derive(Debug, Clone)]
pub struct ILInstruction {
    pub offset: u32,
    pub opcode: ILOpcode,
    pub size: u32,
}

/// CLR value (stack item)
#[derive(Debug, Clone)]
pub enum ClrValue {
    Null,
    Bool(bool),
    Char(char),
    I8(i8),
    U8(u8),
    I16(i16),
    U16(u16),
    I32(i32),
    U32(u32),
    I64(i64),
    U64(u64),
    F32(f32),
    F64(f64),
    IntPtr(i64),
    UIntPtr(u64),
    String(String),
    Object(ObjectHandle),
    Array(ObjectHandle),
    ValueType(Vec<u8>),
}

impl ClrValue {
    pub fn to_i32(&self) -> Option<i32> {
        match self {
            ClrValue::Bool(b) => Some(if *b { 1 } else { 0 }),
            ClrValue::I8(v) => Some(*v as i32),
            ClrValue::U8(v) => Some(*v as i32),
            ClrValue::I16(v) => Some(*v as i32),
            ClrValue::U16(v) => Some(*v as i32),
            ClrValue::I32(v) => Some(*v),
            ClrValue::U32(v) => Some(*v as i32),
            ClrValue::Char(c) => Some(*c as i32),
            _ => None,
        }
    }

    pub fn to_i64(&self) -> Option<i64> {
        match self {
            ClrValue::Bool(b) => Some(if *b { 1 } else { 0 }),
            ClrValue::I8(v) => Some(*v as i64),
            ClrValue::U8(v) => Some(*v as i64),
            ClrValue::I16(v) => Some(*v as i64),
            ClrValue::U16(v) => Some(*v as i64),
            ClrValue::I32(v) => Some(*v as i64),
            ClrValue::U32(v) => Some(*v as i64),
            ClrValue::I64(v) => Some(*v),
            ClrValue::U64(v) => Some(*v as i64),
            ClrValue::IntPtr(v) => Some(*v),
            ClrValue::UIntPtr(v) => Some(*v as i64),
            _ => None,
        }
    }

    pub fn is_null(&self) -> bool {
        matches!(self, ClrValue::Null)
    }

    pub fn is_true(&self) -> bool {
        match self {
            ClrValue::Bool(b) => *b,
            ClrValue::I32(v) => *v != 0,
            ClrValue::I64(v) => *v != 0,
            ClrValue::Object(h) => *h != 0,
            ClrValue::Null => false,
            _ => true,
        }
    }
}

/// Type definition
#[derive(Debug, Clone)]
pub struct TypeDef {
    pub token: TypeToken,
    pub namespace: String,
    pub name: String,
    pub flags: u32,
    pub extends: Option<TypeToken>,
    pub fields: Vec<FieldToken>,
    pub methods: Vec<MethodToken>,
    pub interfaces: Vec<TypeToken>,
    pub is_value_type: bool,
    pub is_enum: bool,
    pub instance_size: usize,
}

impl TypeDef {
    pub fn full_name(&self) -> String {
        if self.namespace.is_empty() {
            self.name.clone()
        } else {
            alloc::format!("{}.{}", self.namespace, self.name)
        }
    }
}

/// Field definition
#[derive(Debug, Clone)]
pub struct FieldDef {
    pub token: FieldToken,
    pub name: String,
    pub flags: u16,
    pub signature: Vec<u8>,
    pub parent: TypeToken,
    pub offset: usize,
    pub field_type: ElementType,
}

/// Method definition
#[derive(Debug, Clone)]
pub struct MethodDef {
    pub token: MethodToken,
    pub name: String,
    pub flags: u16,
    pub impl_flags: u16,
    pub signature: Vec<u8>,
    pub parent: TypeToken,
    pub rva: u32,
    pub params: Vec<u32>,
    pub locals: Vec<ElementType>,
    pub il_code: Vec<u8>,
    pub max_stack: u16,
}

impl MethodDef {
    pub fn is_static(&self) -> bool {
        self.flags & 0x0010 != 0
    }

    pub fn is_virtual(&self) -> bool {
        self.flags & 0x0040 != 0
    }

    pub fn is_abstract(&self) -> bool {
        self.flags & 0x0400 != 0
    }

    pub fn is_special_name(&self) -> bool {
        self.flags & 0x0800 != 0
    }

    pub fn is_constructor(&self) -> bool {
        self.name == ".ctor" || self.name == ".cctor"
    }
}

/// Assembly metadata
#[derive(Debug, Clone)]
pub struct AssemblyMetadata {
    pub name: String,
    pub version: (u16, u16, u16, u16), // Major, Minor, Build, Revision
    pub culture: String,
    pub public_key_token: Option<[u8; 8]>,
    pub flags: u32,
}

/// .NET assembly
pub struct ClrAssembly {
    pub metadata: AssemblyMetadata,
    pub types: BTreeMap<TypeToken, TypeDef>,
    pub fields: BTreeMap<FieldToken, FieldDef>,
    pub methods: BTreeMap<MethodToken, MethodDef>,
    pub strings: BTreeMap<StringToken, String>,
    pub user_strings: BTreeMap<StringToken, String>,
    pub entry_point: Option<MethodToken>,
}

impl ClrAssembly {
    pub fn new(name: &str) -> Self {
        Self {
            metadata: AssemblyMetadata {
                name: String::from(name),
                version: (1, 0, 0, 0),
                culture: String::new(),
                public_key_token: None,
                flags: 0,
            },
            types: BTreeMap::new(),
            fields: BTreeMap::new(),
            methods: BTreeMap::new(),
            strings: BTreeMap::new(),
            user_strings: BTreeMap::new(),
            entry_point: None,
        }
    }

    pub fn get_type(&self, token: TypeToken) -> Option<&TypeDef> {
        self.types.get(&token)
    }

    pub fn get_type_by_name(&self, full_name: &str) -> Option<&TypeDef> {
        self.types.values().find(|t| t.full_name() == full_name)
    }

    pub fn get_method(&self, token: MethodToken) -> Option<&MethodDef> {
        self.methods.get(&token)
    }

    pub fn get_field(&self, token: FieldToken) -> Option<&FieldDef> {
        self.fields.get(&token)
    }

    pub fn get_string(&self, token: StringToken) -> Option<&str> {
        self.strings.get(&token).map(|s| s.as_str())
    }

    pub fn get_user_string(&self, token: StringToken) -> Option<&str> {
        self.user_strings.get(&token).map(|s| s.as_str())
    }
}

/// Managed object
pub struct ManagedObject {
    pub handle: ObjectHandle,
    pub type_token: TypeToken,
    pub fields: Vec<ClrValue>,
    pub sync_block: u32,
}

impl ManagedObject {
    pub fn new(type_token: TypeToken, field_count: usize) -> Self {
        let mut fields = Vec::with_capacity(field_count);
        for _ in 0..field_count {
            fields.push(ClrValue::Null);
        }

        Self {
            handle: next_handle(),
            type_token,
            fields,
            sync_block: 0,
        }
    }
}

/// Managed array
pub struct ManagedArray {
    pub handle: ObjectHandle,
    pub element_type: ElementType,
    pub length: usize,
    pub elements: Vec<ClrValue>,
}

impl ManagedArray {
    pub fn new(element_type: ElementType, length: usize) -> Self {
        let default_value = match element_type {
            ElementType::Boolean => ClrValue::Bool(false),
            ElementType::I1 => ClrValue::I8(0),
            ElementType::U1 => ClrValue::U8(0),
            ElementType::I2 => ClrValue::I16(0),
            ElementType::U2 => ClrValue::U16(0),
            ElementType::I4 => ClrValue::I32(0),
            ElementType::U4 => ClrValue::U32(0),
            ElementType::I8 => ClrValue::I64(0),
            ElementType::U8 => ClrValue::U64(0),
            ElementType::R4 => ClrValue::F32(0.0),
            ElementType::R8 => ClrValue::F64(0.0),
            _ => ClrValue::Null,
        };

        let mut elements = Vec::with_capacity(length);
        for _ in 0..length {
            elements.push(default_value.clone());
        }

        Self {
            handle: next_handle(),
            element_type,
            length,
            elements,
        }
    }
}

/// Execution stack frame
pub struct StackFrame {
    pub method: MethodToken,
    pub ip: u32, // Instruction pointer
    pub locals: Vec<ClrValue>,
    pub args: Vec<ClrValue>,
    pub eval_stack: Vec<ClrValue>,
}

impl StackFrame {
    pub fn new(method: MethodToken, args: Vec<ClrValue>, local_count: usize) -> Self {
        let mut locals = Vec::with_capacity(local_count);
        for _ in 0..local_count {
            locals.push(ClrValue::Null);
        }

        Self {
            method,
            ip: 0,
            locals,
            args,
            eval_stack: Vec::new(),
        }
    }

    pub fn push(&mut self, value: ClrValue) {
        self.eval_stack.push(value);
    }

    pub fn pop(&mut self) -> Option<ClrValue> {
        self.eval_stack.pop()
    }

    pub fn peek(&self) -> Option<&ClrValue> {
        self.eval_stack.last()
    }
}

/// Garbage collector state
pub struct GarbageCollector {
    pub objects: BTreeMap<ObjectHandle, ManagedObject>,
    pub arrays: BTreeMap<ObjectHandle, ManagedArray>,
    pub strings: BTreeMap<ObjectHandle, String>,
    pub total_allocated: usize,
    pub collection_count: u32,
}

impl GarbageCollector {
    pub fn new() -> Self {
        Self {
            objects: BTreeMap::new(),
            arrays: BTreeMap::new(),
            strings: BTreeMap::new(),
            total_allocated: 0,
            collection_count: 0,
        }
    }

    pub fn alloc_object(&mut self, type_token: TypeToken, field_count: usize) -> ObjectHandle {
        let obj = ManagedObject::new(type_token, field_count);
        let handle = obj.handle;
        self.total_allocated += core::mem::size_of::<ManagedObject>() + field_count * 8;
        self.objects.insert(handle, obj);
        handle
    }

    pub fn alloc_array(&mut self, element_type: ElementType, length: usize) -> ObjectHandle {
        let arr = ManagedArray::new(element_type, length);
        let handle = arr.handle;
        self.total_allocated += core::mem::size_of::<ManagedArray>() + length * 8;
        self.arrays.insert(handle, arr);
        handle
    }

    pub fn alloc_string(&mut self, value: String) -> ObjectHandle {
        let handle = next_handle();
        self.total_allocated += value.len();
        self.strings.insert(handle, value);
        handle
    }

    pub fn get_object(&self, handle: ObjectHandle) -> Option<&ManagedObject> {
        self.objects.get(&handle)
    }

    pub fn get_object_mut(&mut self, handle: ObjectHandle) -> Option<&mut ManagedObject> {
        self.objects.get_mut(&handle)
    }

    pub fn get_array(&self, handle: ObjectHandle) -> Option<&ManagedArray> {
        self.arrays.get(&handle)
    }

    pub fn get_array_mut(&mut self, handle: ObjectHandle) -> Option<&mut ManagedArray> {
        self.arrays.get_mut(&handle)
    }

    pub fn get_string(&self, handle: ObjectHandle) -> Option<&str> {
        self.strings.get(&handle).map(|s| s.as_str())
    }

    pub fn collect(&mut self) {
        // Simple collection: for now, just count
        self.collection_count += 1;
        // In real implementation: mark-sweep or generational GC
    }
}

impl Default for GarbageCollector {
    fn default() -> Self {
        Self::new()
    }
}

/// CLR runtime
pub struct ClrRuntime {
    /// Loaded assemblies
    pub assemblies: Vec<ClrAssembly>,
    /// Garbage collector
    pub gc: GarbageCollector,
    /// Call stack
    pub call_stack: Vec<StackFrame>,
    /// Current exception
    pub current_exception: Option<ObjectHandle>,
    /// Initialized
    pub initialized: bool,
}

impl ClrRuntime {
    pub fn new() -> Self {
        Self {
            assemblies: Vec::new(),
            gc: GarbageCollector::new(),
            call_stack: Vec::new(),
            current_exception: None,
            initialized: false,
        }
    }

    /// Load an assembly from PE data
    pub fn load_assembly(&mut self, _data: &[u8]) -> ClrResult<usize> {
        // In real implementation, parse PE and CLI headers
        // For now, create a stub assembly
        let assembly = ClrAssembly::new("StubAssembly");
        let index = self.assemblies.len();
        self.assemblies.push(assembly);
        Ok(index)
    }

    /// Get assembly by index
    pub fn get_assembly(&self, index: usize) -> Option<&ClrAssembly> {
        self.assemblies.get(index)
    }

    /// Execute a method
    pub fn execute(&mut self, method_token: MethodToken) -> ClrResult<Option<ClrValue>> {
        // Find method
        let method = self
            .assemblies
            .iter()
            .find_map(|a| a.get_method(method_token))
            .ok_or(ClrError::MethodNotFound(alloc::format!("{:08X}", method_token)))?
            .clone();

        // Create stack frame
        let frame = StackFrame::new(method_token, Vec::new(), method.locals.len());
        self.call_stack.push(frame);

        // Execute IL
        let result = self.execute_il(&method)?;

        // Pop frame
        self.call_stack.pop();

        Ok(result)
    }

    /// Execute IL code
    fn execute_il(&mut self, method: &MethodDef) -> ClrResult<Option<ClrValue>> {
        let il = &method.il_code;
        let mut ip = 0usize;

        while ip < il.len() {
            let opcode = il[ip];
            ip += 1;

            match opcode {
                0x00 => {} // nop
                0x01 => {} // break
                0x02 => {
                    // ldarg.0
                    let frame = self.call_stack.last_mut().ok_or(ClrError::InvalidIL)?;
                    if let Some(arg) = frame.args.get(0).cloned() {
                        frame.push(arg);
                    }
                }
                0x03 => {
                    // ldarg.1
                    let frame = self.call_stack.last_mut().ok_or(ClrError::InvalidIL)?;
                    if let Some(arg) = frame.args.get(1).cloned() {
                        frame.push(arg);
                    }
                }
                0x06 => {
                    // ldloc.0
                    let frame = self.call_stack.last_mut().ok_or(ClrError::InvalidIL)?;
                    if let Some(local) = frame.locals.get(0).cloned() {
                        frame.push(local);
                    }
                }
                0x0A => {
                    // stloc.0
                    let frame = self.call_stack.last_mut().ok_or(ClrError::InvalidIL)?;
                    if let Some(val) = frame.pop() {
                        if !frame.locals.is_empty() {
                            frame.locals[0] = val;
                        }
                    }
                }
                0x14 => {
                    // ldnull
                    let frame = self.call_stack.last_mut().ok_or(ClrError::InvalidIL)?;
                    frame.push(ClrValue::Null);
                }
                0x15 => {
                    // ldc.i4.m1
                    let frame = self.call_stack.last_mut().ok_or(ClrError::InvalidIL)?;
                    frame.push(ClrValue::I32(-1));
                }
                0x16..=0x1E => {
                    // ldc.i4.0 through ldc.i4.8
                    let val = (opcode - 0x16) as i32;
                    let frame = self.call_stack.last_mut().ok_or(ClrError::InvalidIL)?;
                    frame.push(ClrValue::I32(val));
                }
                0x1F => {
                    // ldc.i4.s
                    if ip >= il.len() {
                        return Err(ClrError::InvalidIL);
                    }
                    let val = il[ip] as i8 as i32;
                    ip += 1;
                    let frame = self.call_stack.last_mut().ok_or(ClrError::InvalidIL)?;
                    frame.push(ClrValue::I32(val));
                }
                0x20 => {
                    // ldc.i4
                    if ip + 4 > il.len() {
                        return Err(ClrError::InvalidIL);
                    }
                    let val = i32::from_le_bytes([il[ip], il[ip + 1], il[ip + 2], il[ip + 3]]);
                    ip += 4;
                    let frame = self.call_stack.last_mut().ok_or(ClrError::InvalidIL)?;
                    frame.push(ClrValue::I32(val));
                }
                0x25 => {
                    // dup
                    let frame = self.call_stack.last_mut().ok_or(ClrError::InvalidIL)?;
                    if let Some(val) = frame.peek().cloned() {
                        frame.push(val);
                    }
                }
                0x26 => {
                    // pop
                    let frame = self.call_stack.last_mut().ok_or(ClrError::InvalidIL)?;
                    frame.pop();
                }
                0x2A => {
                    // ret
                    let frame = self.call_stack.last_mut().ok_or(ClrError::InvalidIL)?;
                    return Ok(frame.pop());
                }
                0x58 => {
                    // add
                    let frame = self.call_stack.last_mut().ok_or(ClrError::InvalidIL)?;
                    let b = frame.pop().and_then(|v| v.to_i32());
                    let a = frame.pop().and_then(|v| v.to_i32());
                    if let (Some(a), Some(b)) = (a, b) {
                        frame.push(ClrValue::I32(a.wrapping_add(b)));
                    }
                }
                0x59 => {
                    // sub
                    let frame = self.call_stack.last_mut().ok_or(ClrError::InvalidIL)?;
                    let b = frame.pop().and_then(|v| v.to_i32());
                    let a = frame.pop().and_then(|v| v.to_i32());
                    if let (Some(a), Some(b)) = (a, b) {
                        frame.push(ClrValue::I32(a.wrapping_sub(b)));
                    }
                }
                0x5A => {
                    // mul
                    let frame = self.call_stack.last_mut().ok_or(ClrError::InvalidIL)?;
                    let b = frame.pop().and_then(|v| v.to_i32());
                    let a = frame.pop().and_then(|v| v.to_i32());
                    if let (Some(a), Some(b)) = (a, b) {
                        frame.push(ClrValue::I32(a.wrapping_mul(b)));
                    }
                }
                0x5B => {
                    // div
                    let frame = self.call_stack.last_mut().ok_or(ClrError::InvalidIL)?;
                    let b = frame.pop().and_then(|v| v.to_i32());
                    let a = frame.pop().and_then(|v| v.to_i32());
                    if let (Some(a), Some(b)) = (a, b) {
                        if b == 0 {
                            return Err(ClrError::DivideByZero);
                        }
                        frame.push(ClrValue::I32(a / b));
                    }
                }
                0xFE => {
                    // Two-byte opcode prefix
                    if ip >= il.len() {
                        return Err(ClrError::InvalidIL);
                    }
                    let op2 = il[ip];
                    ip += 1;
                    match op2 {
                        0x01 => {
                            // ceq
                            let frame = self.call_stack.last_mut().ok_or(ClrError::InvalidIL)?;
                            let b = frame.pop().and_then(|v| v.to_i64());
                            let a = frame.pop().and_then(|v| v.to_i64());
                            if let (Some(a), Some(b)) = (a, b) {
                                frame.push(ClrValue::I32(if a == b { 1 } else { 0 }));
                            }
                        }
                        0x02 => {
                            // cgt
                            let frame = self.call_stack.last_mut().ok_or(ClrError::InvalidIL)?;
                            let b = frame.pop().and_then(|v| v.to_i64());
                            let a = frame.pop().and_then(|v| v.to_i64());
                            if let (Some(a), Some(b)) = (a, b) {
                                frame.push(ClrValue::I32(if a > b { 1 } else { 0 }));
                            }
                        }
                        0x04 => {
                            // clt
                            let frame = self.call_stack.last_mut().ok_or(ClrError::InvalidIL)?;
                            let b = frame.pop().and_then(|v| v.to_i64());
                            let a = frame.pop().and_then(|v| v.to_i64());
                            if let (Some(a), Some(b)) = (a, b) {
                                frame.push(ClrValue::I32(if a < b { 1 } else { 0 }));
                            }
                        }
                        _ => {
                            // Unknown two-byte opcode
                            return Err(ClrError::InvalidIL);
                        }
                    }
                }
                _ => {
                    // Unknown or unimplemented opcode
                    crate::kprintln!("clr: Unimplemented opcode {:02X} at offset {}", opcode, ip - 1);
                    return Err(ClrError::NotImplemented(alloc::format!("opcode {:02X}", opcode)));
                }
            }
        }

        Ok(None)
    }

    /// Initialize the runtime
    pub fn init(&mut self) -> ClrResult<()> {
        // Load mscorlib stubs
        let mut mscorlib = ClrAssembly::new("mscorlib");

        // Add System.Object
        mscorlib.types.insert(
            0x02000001,
            TypeDef {
                token: 0x02000001,
                namespace: String::from("System"),
                name: String::from("Object"),
                flags: 0,
                extends: None,
                fields: Vec::new(),
                methods: vec![0x06000001], // .ctor
                interfaces: Vec::new(),
                is_value_type: false,
                is_enum: false,
                instance_size: 8,
            },
        );

        // Add System.String
        mscorlib.types.insert(
            0x02000002,
            TypeDef {
                token: 0x02000002,
                namespace: String::from("System"),
                name: String::from("String"),
                flags: 0,
                extends: Some(0x02000001),
                fields: Vec::new(),
                methods: Vec::new(),
                interfaces: Vec::new(),
                is_value_type: false,
                is_enum: false,
                instance_size: 16,
            },
        );

        // Add System.Int32
        mscorlib.types.insert(
            0x02000003,
            TypeDef {
                token: 0x02000003,
                namespace: String::from("System"),
                name: String::from("Int32"),
                flags: 0,
                extends: Some(0x02000001),
                fields: Vec::new(),
                methods: Vec::new(),
                interfaces: Vec::new(),
                is_value_type: true,
                is_enum: false,
                instance_size: 4,
            },
        );

        self.assemblies.push(mscorlib);
        self.initialized = true;

        Ok(())
    }

    /// Get statistics
    pub fn stats(&self) -> ClrStats {
        ClrStats {
            assemblies_loaded: self.assemblies.len(),
            objects_allocated: self.gc.objects.len(),
            arrays_allocated: self.gc.arrays.len(),
            strings_allocated: self.gc.strings.len(),
            total_memory: self.gc.total_allocated,
            gc_collections: self.gc.collection_count,
            call_stack_depth: self.call_stack.len(),
        }
    }
}

impl Default for ClrRuntime {
    fn default() -> Self {
        Self::new()
    }
}

/// CLR statistics
#[derive(Debug, Clone)]
pub struct ClrStats {
    pub assemblies_loaded: usize,
    pub objects_allocated: usize,
    pub arrays_allocated: usize,
    pub strings_allocated: usize,
    pub total_memory: usize,
    pub gc_collections: u32,
    pub call_stack_depth: usize,
}

/// Global CLR runtime
static mut CLR_RUNTIME: Option<ClrRuntime> = None;

/// Initialize CLR
pub fn init() {
    let mut runtime = ClrRuntime::new();
    if let Err(e) = runtime.init() {
        crate::kprintln!("clr: Failed to initialize: {:?}", e);
    }

    unsafe {
        CLR_RUNTIME = Some(runtime);
    }

    crate::kprintln!("clr: .NET CLR compatibility layer initialized");
}

/// Get CLR runtime
pub fn runtime() -> &'static mut ClrRuntime {
    unsafe { CLR_RUNTIME.as_mut().expect("CLR not initialized") }
}

/// Format status
pub fn format_status() -> String {
    let stats = runtime().stats();
    alloc::format!(
        ".NET CLR:\n  Assemblies: {}\n  Objects: {}\n  Arrays: {}\n  Strings: {}\n  Memory: {} bytes\n  GC collections: {}",
        stats.assemblies_loaded,
        stats.objects_allocated,
        stats.arrays_allocated,
        stats.strings_allocated,
        stats.total_memory,
        stats.gc_collections
    )
}
