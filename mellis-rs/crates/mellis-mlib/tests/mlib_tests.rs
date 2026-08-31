use std::io::Cursor;
use mellis_mlib::{MlibHeader, MlibReader, MlibWriter, MLIB_MAGIC, MLIB_FORMAT_VERSION};
use mellis_mvir::{Module, Function, BasicBlock, ValueData, Instruction, Terminator, Operand, ValueId, GlobalId, LabelId};
use mellis_semantic::ty::SemanticTypeId;
use mellis_common::SymbolId;

fn dummy_module() -> Module {
    let mut func = Function {
        name: GlobalId { name: "test_func".to_string(), symbol_id: Some(SymbolId(0)) },
        arg_count: 0,
        is_extern: false,
        is_async: false,
        ret_ty: SemanticTypeId(0),
        values: Vec::new(),
        blocks: Vec::new(),
    };
    
    // Add some dummy instructions
    func.values.push(ValueData { inst: Instruction::Alloca, ty: SemanticTypeId(0), span: None });
    func.values.push(ValueData { inst: Instruction::Load { ptr: Operand::Value(ValueId(0)) }, ty: SemanticTypeId(0), span: None });
    
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
    let arena = mellis_ast::AstArena::new();
    let source = "";
    
    let manifest = mellis_mlib::Manifest {
        identity: mellis_mlib::ArtifactIdentity { package_id: "".into(), version: "".into(), module_id: "".into(), artifact_id: "".into() },
        target: mellis_mlib::TargetContract { target_triple: "".into(), object_format: "".into(), abi: "".into(), pointer_width: 64, endianness: "".into() },
        dependencies: mellis_mlib::DependencyTable::default(),
        object_metadata: None,
        provenance: mellis_mlib::Provenance {
            source_fingerprint: [0; 32],
            compiler_version: "".into(),
            codegen_options: "".into(),
            interface_hash: [0; 32],
        },
        export_table: Default::default(),
    };
    MlibWriter::write_module(&module, &arena, &[], source, manifest, None, &mut buffer).expect("write failed");
    
    let mut cursor = Cursor::new(buffer);
    let (mlib_module, _, _) = MlibReader::read_module(&mut cursor).expect("read failed");
    
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
    let result = MlibReader::read_module(&mut cursor).map(|(m, _, _)| m);
    assert!(matches!(result, Err(mellis_mlib::MlibError::VersionMismatch(999))));
}

#[test]
fn test_invalid_magic() {
    let mut buffer = Vec::new();
    let mut header = MlibHeader::new();
    header.magic = *b"XXXX"; // wrong magic
    header.write_to(&mut buffer).unwrap();
    
    let mut cursor = Cursor::new(buffer);
    let result = MlibReader::read_module(&mut cursor).map(|(m, _, _)| m);
    assert!(matches!(result, Err(mellis_mlib::MlibError::InvalidMagic)));
}

#[test]
fn test_corrupted_data() {
    let module = dummy_module();
    let arena = mellis_ast::AstArena::new();
    let source = "";
    let mut buffer = Vec::new();
    let manifest = mellis_mlib::Manifest {
        identity: mellis_mlib::ArtifactIdentity { package_id: "".into(), version: "".into(), module_id: "".into(), artifact_id: "".into() },
        target: mellis_mlib::TargetContract { target_triple: "".into(), object_format: "".into(), abi: "".into(), pointer_width: 64, endianness: "".into() },
        dependencies: mellis_mlib::DependencyTable::default(),
        object_metadata: None,
        provenance: mellis_mlib::Provenance {
            source_fingerprint: [0; 32],
            compiler_version: "".into(),
            codegen_options: "".into(),
            interface_hash: [0; 32],
        },
        export_table: Default::default(),
    };
    MlibWriter::write_module(&module, &arena, &[], source, manifest, None, &mut buffer).unwrap();
    
    // truncate buffer
    buffer.truncate(buffer.len() / 2);
    
    let mut cursor = Cursor::new(buffer);
    let result = MlibReader::read_module(&mut cursor).map(|(m, _, _)| m);
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
    let result = MlibReader::read_module(&mut cursor).map(|(m, _, _)| m);
    // The reader errors because of Unknown section during reading the entry
    assert!(result.is_err());
}

#[test]
fn test_load_core_mlib() {
    use std::fs::File;
    let mut f = File::open("../../../lib/core.mlib").unwrap();
    let m = mellis_mlib::MlibReader::read_module(&mut f).map(|(m, _, _)| m);
    println!("{:?}", m);
    m.unwrap();
}
