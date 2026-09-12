use std::io::Cursor;
use luna_llib::{MlibHeader, MlibReader, MlibWriter, MLIB_MAGIC, MLIB_FORMAT_VERSION};
use luna_mvir::{ValueOrigin, Module, Function, BasicBlock, ValueData, Instruction, Terminator, Operand, ValueId, GlobalId, LabelId};
use luna_semantic::ty::SemanticTypeId;
use luna_common::SymbolId;

fn dummy_module() -> Module {
    let mut func = Function {
        name: GlobalId { name: "test_func".to_string(), symbol_id: Some(SymbolId(0)) },
        arg_count: 0,
        link_name: None,
        param_types: vec![],
        is_extern: false,
        is_async: false,
        ret_ty: SemanticTypeId(0),
        values: Vec::new(),
        blocks: Vec::new(),
    };
    
    // Add some dummy instructions
    func.values.push(ValueData { inst: Instruction::Alloca, ty: SemanticTypeId(0), span: None, origin: ValueOrigin::Temporary });
    func.values.push(ValueData { inst: Instruction::Load { ptr: Operand::Value(ValueId(0)) }, ty: SemanticTypeId(0), span: None, origin: ValueOrigin::Temporary });
    
    // Add block
    func.blocks.push(BasicBlock {
        label: LabelId { name: "entry0".to_string() },
        insts: vec![ValueId(0), ValueId(1)],
        terminator: Some(Terminator::Ret { value: Some(Operand::Value(ValueId(1))) }),
    });

    let mut module = Module::new();
    module.functions.push(func);
    module
}

#[test]
fn test_golden_roundtrip() {
    let module = dummy_module();
    let mut buffer = Vec::new();
    let arena = luna_ast::AstArena::new();
    let source = "";
    
    let manifest = luna_llib::Manifest {
        identity: luna_llib::ArtifactIdentity { package_id: "".into(), version: "".into(), module_id: "".into(), artifact_id: "".into() },
        target: luna_llib::TargetContract { target_triple: "".into(), object_format: "".into(), abi: "".into(), pointer_width: 64, endianness: "".into() },
        dependencies: luna_llib::DependencyTable::default(),
        object_metadata: None,
        provenance: luna_llib::Provenance {
            source_fingerprint: [0; 32],
            compiler_version: "".into(),
            codegen_options: "".into(),
            interface_hash: [0; 32],
        },
        export_table: Default::default(),
    };
    MlibWriter::write_module(&module, &arena, &[], source, manifest, None, None, &mut buffer).expect("write failed");
    
    let mut cursor = Cursor::new(buffer);
    let (mlib_module, _, _, _) = MlibReader::read_module(&mut cursor).expect("read failed");
    
    assert_eq!(mlib_module.functions.len(), 1);
    assert_eq!(mlib_module.functions[0].name, "test_func");
    assert_eq!(mlib_module.functions[0].values.len(), 2);
}

#[test]
fn test_version_mismatch() {
    let mut buffer = Vec::new();
    let mut header = MlibHeader::new();
    header.format_version = 999; // wrong version
    header.write_to(&mut buffer).unwrap();
    
    let mut cursor = Cursor::new(buffer);
    let result = MlibReader::read_module(&mut cursor).map(|(m, _, _, _)| m);
    assert!(matches!(result, Err(luna_llib::MlibError::VersionMismatch(999))));
}

#[test]
fn test_invalid_magic() {
    let mut buffer = Vec::new();
    let mut header = MlibHeader::new();
    header.magic = *b"XXXX"; // wrong magic
    header.write_to(&mut buffer).unwrap();
    
    let mut cursor = Cursor::new(buffer);
    let result = MlibReader::read_module(&mut cursor).map(|(m, _, _, _)| m);
    assert!(matches!(result, Err(luna_llib::MlibError::InvalidMagic)));
}

#[test]
fn test_corrupted_data() {
    let module = dummy_module();
    let arena = luna_ast::AstArena::new();
    let source = "";
    let mut buffer = Vec::new();
    let manifest = luna_llib::Manifest {
        identity: luna_llib::ArtifactIdentity { package_id: "".into(), version: "".into(), module_id: "".into(), artifact_id: "".into() },
        target: luna_llib::TargetContract { target_triple: "".into(), object_format: "".into(), abi: "".into(), pointer_width: 64, endianness: "".into() },
        dependencies: luna_llib::DependencyTable::default(),
        object_metadata: None,
        provenance: luna_llib::Provenance {
            source_fingerprint: [0; 32],
            compiler_version: "".into(),
            codegen_options: "".into(),
            interface_hash: [0; 32],
        },
        export_table: Default::default(),
    };
    MlibWriter::write_module(&module, &arena, &[], source, manifest, None, None, &mut buffer).unwrap();
    
    // truncate buffer
    buffer.truncate(buffer.len() / 2);
    
    let mut cursor = Cursor::new(buffer);
    let result = MlibReader::read_module(&mut cursor).map(|(m, _, _, _)| m);
    assert!(result.is_err()); // Either Io or BincodeError
}

#[test]
fn test_unknown_section() {
    let mut buffer = Vec::new();
    let mut header = MlibHeader::new();
    header.section_count = 1;
    header.write_to(&mut buffer).unwrap();
    
    // Write an unknown section ID 0xFFFFFFFF
    buffer.extend_from_slice(&0xFFFFFFFFu32.to_le_bytes()); // section_type
    buffer.extend_from_slice(&0u64.to_le_bytes()); // offset
    buffer.extend_from_slice(&0u64.to_le_bytes()); // length
    
    let mut cursor = Cursor::new(buffer);
    let result = MlibReader::read_module(&mut cursor).map(|(m, _, _, _)| m);
    // The reader errors because of Unknown section during reading the entry
    assert!(result.is_err());
}

#[test]
fn test_load_core_mlib() {
    use std::fs::File;
    let path = "../../../lib/core.llib";
    let fallback = "../../../lib/core.mlib";
    let mut f = match File::open(path).or_else(|_| File::open(fallback)) {
        Ok(f) => f,
        Err(_) => return, // core library not yet built, skip test
    };
    let m = luna_llib::MlibReader::read_module(&mut f).map(|(m, _, _, _)| m);
    println!("{:?}", m);
    let _ = m.ok();
}
