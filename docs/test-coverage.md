# Test coverage

Avro 1.12.2's own Java tests (`upstream/lang/java/avro/src/test/java/org/apache/avro/`), mapped to their ports.

The classes that reach the ported code are listed method by method. The classes that test something the
port leaves out are listed by class. Upstream builds many schemas with `SchemaBuilder` or `Schema.create*`; the
ports give the same schema as JSON, and read into and back from binary where upstream inspects a
`GenericRecord`.

139 test methods are ported, and 82 in these classes are out of scope.

There are also differential vectors in `tests/vectors/*.tsv`: schema, payload and outcome, recorded from Avro 1.12.2
itself by `scripts/record.sh`. They cover the parser, the JSON decoder and the binary decoder much more densely than
the unit tests do.

| Upstream test | Port |
|---|---|
| TestSchema#splitSchemaBuild | `tests/upstream_schema.rs::split_schema_build` |
| TestSchema#defaultRecordWithDuplicateFieldName | `tests/upstream_schema.rs::default_record_with_duplicate_field_name` |
| TestSchema#createUnionVarargs | out of scope: `Schema.createUnion` (programmatic construction) |
| TestSchema#recordWithNullDoc | `tests/upstream_schema.rs::record_with_null_doc` |
| TestSchema#recordWithNullNamespace | `tests/upstream_schema.rs::record_with_null_namespace` |
| TestSchema#emptyRecordSchema | `tests/upstream_schema.rs::empty_record_schema` |
| TestSchema#parseEmptySchema | `tests/upstream_schema.rs::parse_empty_schema` |
| TestSchema#schemaWithFields | `tests/upstream_schema.rs::schema_with_fields` |
| TestSchema#schemaWithNullFields | `tests/upstream_schema.rs::schema_with_null_fields` |
| TestSchema#isUnionOnUnionWithMultipleElements | `tests/upstream_schema.rs::is_union_on_union_with_multiple_elements` |
| TestSchema#isUnionOnUnionWithOneElement | `tests/upstream_schema.rs::is_union_on_union_with_one_element` |
| TestSchema#isUnionOnRecord | `tests/upstream_schema.rs::is_union_on_record` |
| TestSchema#isUnionOnArray | `tests/upstream_schema.rs::is_union_on_array` |
| TestSchema#isUnionOnEnum | `tests/upstream_schema.rs::is_union_on_enum` |
| TestSchema#isUnionOnFixed | `tests/upstream_schema.rs::is_union_on_fixed` |
| TestSchema#isUnionOnMap | `tests/upstream_schema.rs::is_union_on_map` |
| TestSchema#isNullableOnUnionWithNull | `tests/upstream_schema.rs::is_nullable_on_union_with_null` |
| TestSchema#isNullableOnUnionWithoutNull | `tests/upstream_schema.rs::is_nullable_on_union_without_null` |
| TestSchema#isNullableOnRecord | `tests/upstream_schema.rs::is_nullable_on_record` |
| TestSchema#serialization | `tests/upstream_schema.rs::serialization` |
| TestSchema#reconstructSchemaStringWithoutInlinedChildReference | `tests/upstream_schema.rs::reconstruct_schema_string_without_inlined_child_reference` |
| TestSchema#intDefaultValue | `tests/upstream_schema.rs::int_default_value` |
| TestSchema#validLongAsIntDefaultValue | `tests/upstream_schema.rs::valid_long_as_int_default_value` |
| TestSchema#invalidLongAsIntDefaultValue | `tests/upstream_schema.rs::invalid_long_as_int_default_value` |
| TestSchema#doubleAsIntDefaultValue | `tests/upstream_schema.rs::double_as_int_default_value` |
| TestSchema#longDefaultValue | `tests/upstream_schema.rs::long_default_value` |
| TestSchema#intAsLongDefaultValue | `tests/upstream_schema.rs::int_as_long_default_value` |
| TestSchema#doubleAsLongDefaultValue | `tests/upstream_schema.rs::double_as_long_default_value` |
| TestSchema#doubleDefaultValue | `tests/upstream_schema.rs::double_default_value` |
| TestSchema#intAsDoubleDefaultValue | `tests/upstream_schema.rs::int_as_double_default_value` |
| TestSchema#longAsDoubleDefaultValue | `tests/upstream_schema.rs::long_as_double_default_value` |
| TestSchema#floatAsDoubleDefaultValue | `tests/upstream_schema.rs::float_as_double_default_value` |
| TestSchema#floatDefaultValue | `tests/upstream_schema.rs::float_default_value` |
| TestSchema#intAsFloatDefaultValue | `tests/upstream_schema.rs::int_as_float_default_value` |
| TestSchema#longAsFloatDefaultValue | `tests/upstream_schema.rs::long_as_float_default_value` |
| TestSchema#doubleAsFloatDefaultValue | `tests/upstream_schema.rs::double_as_float_default_value` |
| TestSchema#enumSymbolAsNull | `tests/upstream_schema.rs::enum_symbol_as_null` |
| TestSchema#schemaFieldWithoutSchema | `tests/upstream_schema.rs::schema_field_without_schema` |
| TestSchema#parseRecordWithNameAsType | `tests/upstream_schema.rs::parse_record_with_name_as_type` |
| TestSchema#qualifiedName | `tests/upstream_schema.rs::qualified_name` |
| TestSchema#validValue | `tests/upstream_schema.rs::valid_value` |
| TestSchema#enumLateDefine | `tests/upstream_schema.rs::enum_late_define` |
| TestSchema#testRecordInArray | `tests/upstream_schema.rs::record_in_array` |
| TestSchema#testUnionFieldType | `tests/upstream_schema.rs::union_field_type` |
| TestSchema#parseAliases | `tests/upstream_schema.rs::parse_aliases` |
| TestSchema#testContentAfterAvsc | `tests/upstream_schema.rs::test_content_after_avsc` |
| TestSchema#testContentAfterAvscInInputStream | out of scope: `Schema.Parser.parse(InputStream)` allows content after the schema; only `parse(String)` is ported |
| TestSchema#testContentAfterAvscInFile | out of scope: `parse(File)` |
| TestSchema#testParseMultipleFile | `tests/upstream_schema.rs::test_parse_multiple_file` |
| TestSchema#add_types | `tests/upstream_schema.rs::add_types` |
| TestSchema#testParserNullValidate | `tests/upstream_schema.rs::test_parser_null_validate` |
| TestSchema#disallowTypeObjectForNamedType | `tests/upstream_schema.rs::disallow_type_object_for_named_type` |
| TestSchema#enumWriteUnknownField | out of scope: writes a hand-built `GenericRecord`; the writer here only sees datums the reader built (an unknown symbol in JSON fails in the decoder: `json.tsv`) |
| TestSchemaParser#testStaticParseText | `tests/upstream_schema.rs::test_static_parse_text` |
| TestSchemaParser#testStaticParsePath | out of scope: `parse(Path)` |
| TestSchemaParser#testParseFile | out of scope: `parse(File)` |
| TestSchemaParser#testParsePath | out of scope: `parse(Path)` |
| TestSchemaParser#testParseURI | out of scope: `parse(URI)` with charsets |
| TestSchemaParser#testParseReader | out of scope: `parse(Reader)` |
| TestSchemaParser#testParseStream | out of scope: `parse(InputStream)` |
| TestSchemaParser#testParseTextWithFallbackJsonParser | `tests/upstream_schema.rs::test_parse_text_with_fallback_json_parser` |
| TestSchemaParser#testParseByCustomParser | out of scope: pluggable `FormattedSchemaParser`s |
| TestSchemaParser#testSingleParseError | `tests/upstream_schema.rs::test_single_parse_error` |
| TestSchemaParser#testMultipleParseErrors | out of scope: pluggable `FormattedSchemaParser`s |
| TestSchemaParser#testIOFailureWhileParsingText | out of scope: pluggable `FormattedSchemaParser`s |
| TestFixed#fixedDefaultValueDrop | `tests/upstream_schema.rs::fixed_default_value_drop` |
| TestFixed#fixedLengthOutOfLimit | `tests/upstream_schema.rs::fixed_length_out_of_limit` |
| TestFixed#fixedNegativeLength | `tests/upstream_schema.rs::fixed_negative_length` |
| TestUnionSelfReference#selfReferenceInUnion | `tests/upstream_schema.rs::self_reference_in_union` |
| TestUnionSelfReference#selfReferenceInThreeUnion | `tests/upstream_schema.rs::self_reference_in_three_union` |
| SchemaNameValidatorTest#validator | `tests/upstream_schema.rs::validator` |
| ParseContextTest#checkNewNameContextContainsPrimitives | `tests/upstream_schema.rs::check_new_name_context_contains_primitives` |
| ParseContextTest#primitivesAreNotCached | `tests/upstream_schema.rs::primitives_are_not_cached` |
| ParseContextTest#validateSchemaRetrievalFailure | `tests/upstream_schema.rs::validate_schema_retrieval_failure` |
| ParseContextTest#validateSchemaRetrievalByFullName | `tests/upstream_schema.rs::validate_schema_retrieval_by_full_name` |
| ParseContextTest#validateSchemaRetrievalBySimpleName | `tests/upstream_schema.rs::validate_schema_retrieval_by_simple_name` |
| ParseContextTest#verifyPutIsIdempotent | `tests/upstream_schema.rs::verify_put_is_idempotent` |
| ParseContextTest#verifyPutOnlyAcceptsNamedSchemas | out of scope: `ParseContext.put` of an unnamed schema has no JSON form |
| ParseContextTest#verifyAddDoesNotAllowChangingSchemas | `tests/upstream_schema.rs::verify_add_does_not_allow_changing_schemas` |
| TestSchemaResolver#testResolving | `tests/upstream_schema.rs::test_resolving` |
| TestSchemaResolver#testIsUnresolvedSchemaError1 | out of scope: `SchemaResolver.getUnresolvedSchemaName` API |
| TestSchemaResolver#testIsUnresolvedSchemaError2 | out of scope: `SchemaResolver.getUnresolvedSchemaName` API |
| TestSchemaResolver#testIsUnresolvedSchemaError3 | out of scope: `SchemaResolver.getUnresolvedSchemaName` API |
| TestSchemaResolver#testGetUnresolvedSchemaNameError | out of scope: `SchemaResolver.getUnresolvedSchemaName` API |
| TestNestedRecords#singleSubRecord | `tests/upstream_schema.rs::single_sub_record` |
| TestNestedRecords#singleSubRecordExtraField | `tests/upstream_schema.rs::single_sub_record_extra_field` |
| SchemaFormatterTest#validateDefaultNaming | out of scope: `SchemaFormatterFactory` API |
| SchemaFormatterTest#validateJsonFormatDefaultsToPrettyPrinting | out of scope: pretty printing (`json/pretty`) |
| SchemaFormatterTest#validateSupportForPrettyJsonFormat | out of scope: pretty printing (`json/pretty`) |
| SchemaFormatterTest#validateSupportForInlineJsonFormat | `tests/upstream_schema.rs::validate_support_for_inline_json_format` |
| SchemaFormatterTest#checkThatJsonHasNoExtraVariant | out of scope: `SchemaFormatter` API |
| SchemaFormatterTest#validateSupportForCanonicalFormat | out of scope: Parsing Canonical Form |
| SchemaFormatterTest#checkThatCanonicalFormHasNoVariants | out of scope: `SchemaFormatter` API |
| SchemaFormatterTest#checkExceptionForMissingFormat | out of scope: `SchemaFormatter` API |
| TestSchemaCommons#runFolder | `tests/vectors.rs::shared_schemas` (the parse; the data files are containers, out of scope) |
| TestJsonDecoder#testInt | `tests/upstream_io.rs::test_int` |
| TestJsonDecoder#testLong | `tests/upstream_io.rs::test_long` |
| TestJsonDecoder#testFloat | `tests/upstream_io.rs::test_float` |
| TestJsonDecoder#testDouble | `tests/upstream_io.rs::test_double` |
| TestJsonDecoder#reorderFields | `tests/upstream_io.rs::reorder_fields` |
| TestJsonDecoder#testIntWithError | `tests/upstream_io.rs::test_int_with_error` |
| TestJsonDecoder#testIeee754SpecialCases | `tests/upstream_io.rs::test_ieee754_special_cases` |
| TestEncoders#binaryEncoderInit | out of scope: encoder construction |
| TestEncoders#badBinaryEncoderInit | out of scope: encoder construction |
| TestEncoders#blockingBinaryEncoderInit | out of scope: encoder construction |
| TestEncoders#badBlockintBinaryEncoderInit | out of scope: encoder construction |
| TestEncoders#directBinaryEncoderInit | out of scope: encoder construction |
| TestEncoders#badDirectBinaryEncoderInit | out of scope: encoder construction |
| TestEncoders#blockingDirectBinaryEncoderInit | out of scope: encoder construction |
| TestEncoders#badBlockingDirectBinaryEncoderInit | out of scope: encoder construction |
| TestEncoders#jsonEncoderInit | out of scope: encoder construction |
| TestEncoders#badJsonEncoderInitOS | out of scope: encoder construction |
| TestEncoders#badJsonEncoderInit | out of scope: encoder construction |
| TestEncoders#jsonEncoderNewlineDelimited | out of scope: several datums through one encoder |
| TestEncoders#jsonEncoderWhenIncludeNamespaceOptionIsFalse | out of scope: `JsonEncoder.setIncludeNamespace(false)` (MockServer keeps the default) |
| TestEncoders#jsonEncoderWhenIncludeNamespaceOptionIsTrue | `tests/upstream_io.rs::json_encoder_when_include_namespace_option_is_true` |
| TestEncoders#validatingEncoderInit | out of scope: `ValidatingEncoder` |
| TestEncoders#jsonRecordOrdering | `tests/upstream_io.rs::json_record_ordering` |
| TestEncoders#jsonExcessFields | `tests/upstream_io.rs::json_excess_fields` |
| TestEncoders#jsonRecordOrdering2 | `tests/upstream_io.rs::json_record_ordering2` |
| TestEncoders#jsonRecordOrderingWithProjection | out of scope: reading with a different reader schema |
| TestEncoders#jsonRecordOrderingWithProjection2 | out of scope: reading with a different reader schema |
| TestEncoders#arrayBackedByteBuffer | `tests/upstream_io.rs::array_backed_byte_buffer` |
| TestEncoders#mappedByteBuffer | out of scope: `ByteBuffer` sources |
| TestEncoders#testJsonEncoderInitAutoFlush | out of scope: encoder configuration and flushing |
| TestEncoders#testJsonEncoderInitAutoFlushDisabled | out of scope: encoder configuration and flushing |
| TestGenericDatumReader#readerCache | out of scope: resolver caching |
| TestGenericDatumReader#newInstanceFromString | out of scope: `java-class` string types |
| TestGenericDatumReader#testMinBytesPerElementPrimitives | `tests/upstream_io.rs::test_min_bytes_per_element_primitives` |
| TestGenericDatumReader#testMinBytesPerElementFixed | `tests/upstream_io.rs::test_min_bytes_per_element_fixed` |
| TestGenericDatumReader#testMinBytesPerElementUnion | `tests/upstream_io.rs::test_min_bytes_per_element_union` |
| TestGenericDatumReader#testMinBytesPerElementRecord | `tests/upstream_io.rs::test_min_bytes_per_element_record` |
| TestGenericDatumReader#testMinBytesPerElementNestedCollections | `tests/upstream_io.rs::test_min_bytes_per_element_nested_collections` |
| TestGenericDatumReader#arrayOfIntsRejectsHugeCount | `tests/upstream_io.rs::array_of_ints_rejects_huge_count` |
| TestGenericDatumReader#arrayHugeCountOnStreamClampsPreallocation | out of scope: `InputStream` sources (arrays are not preallocated here at all) |
| TestGenericDatumReader#arrayOfNullsAcceptsLargeCount | `tests/upstream_io.rs::array_of_nulls_accepts_large_count` |
| TestGenericDatumReader#mapOfStringToIntRejectsHugeCount | `tests/upstream_io.rs::map_of_string_to_int_rejects_huge_count` |
| TestGenericDatumReader#mapOfStringToNullRejectsHugeCount | `tests/upstream_io.rs::map_of_string_to_null_rejects_huge_count` |
| TestGenericDatumReader#arrayOfZeroLengthFixedAcceptsLargeCount | `tests/upstream_io.rs::array_of_zero_length_fixed_accepts_large_count` |
| TestGenericDatumReader#arrayOfRecordsRejectsHugeCountUsingFullRecordSize | `tests/upstream_io.rs::array_of_records_rejects_huge_count_using_full_record_size` |
| TestGenericDatumReader#mapOfRecordsRejectsHugeCountUsingFullRecordSize | `tests/upstream_io.rs::map_of_records_rejects_huge_count_using_full_record_size` |
| TestGenericDatumReader#arrayOfNullsRejectsCountAboveAllocationLimit | `tests/upstream_io.rs::array_of_nulls_rejects_count_above_allocation_limit` |
| TestGenericDatumReader#arrayOfNullsWithinAllocationLimitStillDecodes | `tests/upstream_io.rs::array_of_nulls_within_allocation_limit_still_decodes` |
| TestGenericDatumReader#arrayOfAllNullRecordsRejectsCountAboveAllocationLimit | `tests/upstream_io.rs::array_of_all_null_records_rejects_count_above_allocation_limit` |
| TestGenericDatumReader#recordOfNullArrayFieldsRejectedCumulativelyAcrossDatum | `tests/upstream_io.rs::record_of_null_array_fields_rejected_cumulatively_across_datum` |
| TestGenericDatumReader#recordOfNullArrayFieldsWithinCumulativeLimitStillDecodes | `tests/upstream_io.rs::record_of_null_array_fields_within_cumulative_limit_still_decodes` |
| TestGenericDatumReader#hugeCollectionsRejectedOnBothReaderPaths | `tests/upstream_io.rs::huge_collections_rejected_on_both_reader_paths` |
| TestGenericDatumReader#fastAndClassicReaderRejectCumulativeNullBlocks | `tests/upstream_io.rs::fast_and_classic_reader_reject_cumulative_null_blocks` |
| TestGenericDatumReader#fastAndClassicReaderRejectNegativeNullBlockCount | `tests/upstream_io.rs::fast_and_classic_reader_reject_negative_null_block_count` |
| TestGenericDatumReader#fastAndClassicReaderRejectMinValueBlockCount | `tests/upstream_io.rs::fast_and_classic_reader_reject_min_value_block_count` |
| TestGenericDatumReader#fastAndClassicReaderRejectOutOfRangeUnionIndex | `tests/upstream_io.rs::fast_and_classic_reader_reject_out_of_range_union_index` |
| TestGenericDatumReader#fastAndClassicReaderRejectOutOfRangeEnumIndex | `tests/upstream_io.rs::fast_and_classic_reader_reject_out_of_range_enum_index` |
| TestGenericDatumReader#fastAndClassicReaderDecodeSmallNullArray | `tests/upstream_io.rs::fast_and_classic_reader_decode_small_null_array` |
| TestGenericDatumReader#skipArrayOfNullRejectsHugeCount | out of scope: `GenericDatumReader.skip` |
| TestGenericDatumReader#skipSmallNullArraySucceeds | out of scope: `GenericDatumReader.skip` |
| TestGenericDatumReader#skipMapRejectsHugeCount | out of scope: `GenericDatumReader.skip` |
| TestGenericDatumReader#resolvingSkipOfHugeNullArrayFieldIsBounded | out of scope: reading with a different reader schema |
| TestGenericDatumWriter#unionUnresolvedExceptionExplicitWhichField | out of scope: hand-built datums |
| TestGenericDatumWriter#write | `tests/upstream_io.rs::write` |
| TestGenericDatumWriter#arrayConcurrentModification | out of scope: hand-built datums |
| TestGenericDatumWriter#mapConcurrentModification | out of scope: hand-built datums |
| TestGenericDatumWriter#allowWritingPrimitives | `tests/upstream_io.rs::allow_writing_primitives` |
| TestGenericDatumWriter#writeDoesNotAllowStringForGenericEnum | out of scope: hand-built datums |
| TestGenericDatumWriter#writeDoesNotAllowJavaEnumForGenericEnum | out of scope: hand-built datums |
| TestGenericDatumWriter#writeFieldWithDefaultWithExplicitNullDefaultInSchema | out of scope: hand-built datums |
| TestGenericDatumWriter#writeFieldWithDefaultWithoutExplicitNullDefaultInSchema | out of scope: hand-built datums |
| TestGenericDatumWriter#nestedNPEErrorClarity | out of scope: hand-built datums |
| TestGenericDatumWriter#nPEForMapKeyErrorClarity | out of scope: hand-built datums |
| TestGenericDatumWriter#shortPathNPEErrorClarity | out of scope: hand-built datums |
| TestGenericDatumWriter#nestedCCEErrorClarity | out of scope: hand-built datums |
| TestGenericDatumWriter#shortPathCCEErrorClarity | out of scope: hand-built datums |
| TestGenericDatumWriter#nestedATEErrorClarity | out of scope: hand-built datums |
| TestBinaryDecoder#eofBoolean | `src/binary.rs::eof_boolean_int_long_float_double_bytes_string_fixed_enum` |
| TestBinaryDecoder#eofInt | `src/binary.rs::eof_boolean_int_long_float_double_bytes_string_fixed_enum` |
| TestBinaryDecoder#eofLong | `src/binary.rs::eof_boolean_int_long_float_double_bytes_string_fixed_enum` |
| TestBinaryDecoder#eofFloat | `src/binary.rs::eof_boolean_int_long_float_double_bytes_string_fixed_enum` |
| TestBinaryDecoder#eofDouble | `src/binary.rs::eof_boolean_int_long_float_double_bytes_string_fixed_enum` |
| TestBinaryDecoder#eofBytes | `src/binary.rs::eof_boolean_int_long_float_double_bytes_string_fixed_enum` |
| TestBinaryDecoder#eofString | `src/binary.rs::eof_boolean_int_long_float_double_bytes_string_fixed_enum` |
| TestBinaryDecoder#eofFixed | `src/binary.rs::eof_boolean_int_long_float_double_bytes_string_fixed_enum` |
| TestBinaryDecoder#eofEnum | `src/binary.rs::eof_boolean_int_long_float_double_bytes_string_fixed_enum` |
| TestBinaryDecoder#reuse | out of scope: decoder reuse |
| TestBinaryDecoder#decodeFromSources | out of scope: `InputStream` and direct decoders |
| TestBinaryDecoder#inputStreamProxy | out of scope: `BinaryDecoder.inputStream()` |
| TestBinaryDecoder#inputStreamProxyDetached | out of scope: `BinaryDecoder.inputStream()` |
| TestBinaryDecoder#inputStreamPartiallyUsed | out of scope: `BinaryDecoder.inputStream()` |
| TestBinaryDecoder#badIntEncoding | `src/binary.rs::bad_int_encoding` |
| TestBinaryDecoder#badLongEncoding | `src/binary.rs::bad_long_encoding` |
| TestBinaryDecoder#testStringNegativeLength | `src/binary.rs::test_string_negative_length` |
| TestBinaryDecoder#testStringVmMaxSize | `src/binary.rs::test_string_vm_max_size` |
| TestBinaryDecoder#testStringMaxCustom | out of scope: limits set by system property |
| TestBinaryDecoder#testStringLengthExceedsAvailableBytes | `src/binary.rs::test_string_length_exceeds_available_bytes` |
| TestBinaryDecoder#testBytesLengthExceedsAvailableBytes | `src/binary.rs::test_bytes_length_exceeds_available_bytes` |
| TestBinaryDecoder#testStringLengthDoesNotTrustUnknownAvailable | out of scope: `InputStream` sources |
| TestBinaryDecoder#testBytesNegativeLength | `src/binary.rs::test_bytes_negative_length` |
| TestBinaryDecoder#testBytesVmMaxSize | `src/binary.rs::test_bytes_vm_max_size` |
| TestBinaryDecoder#testBytesMaxCustom | out of scope: limits set by system property |
| TestBinaryDecoder#testArrayVmMaxSize | `src/binary.rs::test_array_vm_max_size` |
| TestBinaryDecoder#testArrayMaxCustom | out of scope: limits set by system property |
| TestBinaryDecoder#testMapVmMaxSize | `src/binary.rs::test_map_vm_max_size` |
| TestBinaryDecoder#testMapMaxCustom | out of scope: limits set by system property |
| TestBinaryDecoder#longLengthEncoding | `src/binary.rs::long_length_encoding` |
| TestBinaryDecoder#intTooShort | `src/binary.rs::int_too_short` |
| TestBinaryDecoder#longTooShort | `src/binary.rs::long_too_short` |
| TestBinaryDecoder#floatTooShort | `src/binary.rs::float_too_short` |
| TestBinaryDecoder#doubleTooShort | `src/binary.rs::double_too_short` |
| TestBinaryDecoder#skipping | out of scope: `skip*` methods (nothing is skipped with one schema) |
| TestBinaryDecoder#eof | `src/binary.rs::eof` |
| TestSystemLimitException#testCheckMaxBytesLength | `src/binary.rs::test_check_max_bytes_length` |
| TestSystemLimitException#testCheckMaxCollectionLengthFromZero | `src/binary.rs::test_check_max_collection_length_from_zero` |
| TestSystemLimitException#testCheckMaxStringLength | `src/binary.rs::test_check_max_string_length` |
| TestSystemLimitException#testCheckMaxCollectionAllocation | out of scope: limits set by system property (the cap is `MAX_ZERO_BYTE_COLLECTION_ALLOCATION`, see `upstream_io.rs`) |
| TestSystemLimitException#testCheckMaxCollectionAllocationDefaultsToHeapFraction | out of scope: the heap-derived default (a constant here, README) |
| TestSystemLimitException#testCheckMaxCollectionLengthFromNonZero | `src/binary.rs::test_check_max_collection_length_from_non_zero` |
| TestBinaryEncoderFidelity#binaryEncoder | `src/binary.rs::binary_encoder` |
| TestBinaryEncoderFidelity#directBinaryEncoder | out of scope: one encoder here (`binary_encoder` covers the output) |
| TestBinaryEncoderFidelity#blockingDirectBinaryEncoder | out of scope: blocking encoders |
| TestBinaryEncoderFidelity#blockingBinaryEncoder | out of scope: blocking encoders |
| TestBinaryData#skipLong | out of scope: `BinaryData` utilities |
| TestBinaryData#testIntLongVleEquality | `src/binary.rs::test_int_long_vle_equality` |
| TestBinaryData#testCompareBytesUnsigned | out of scope: `BinaryData.compareBytes` |

## Out of scope, by class

| Upstream tests | Why |
|---|---|
| TestSchemaBuilder, TestSchemaValidation | `SchemaBuilder` and `SchemaValidator` APIs; the schemas they build are what the parser tests cover |
| TestSchemaCompatibility*, TestResolver, TestReadingWritingDataInEvolvedSchemas, TestSchemaValidateDefault, TestUnionError, io/TestResolvingIO*, io/parsing/TestResolvingGrammarGenerator* | schema resolution between different writer and reader schemas |
| TestSchemaNormalization, SchemaFormatterTest (canonical) | Parsing Canonical Form and fingerprints |
| TestLogicalType, TestDecimalConversion, TestBigDecimalConversion, TestUuidConversions, generic/TestGenericLogicalTypes, TestCircularReferences | logical-type conversions (a `logicalType` is a property here, as in `GenericData` without conversions) |
| TestDataFile*, file/*, TestSchemaCommons (data files) | object container files and codecs |
| TestProtocol, message/* | protocols and single-object encoding |
| specific/*, reflect/*, util/springframework/*, util/TestClassSecurityValidator | specific and reflect data |
| generic/TestGenericData, generic/GenericDataTest, generic/GenericDataArrayTest, generic/PrimitivesArraysTest, generic/TestGenericRecordBuilder, generic/TestGenericConcreteEnum, generic/TestSkipEnumSchema, TestCompare | the `GenericData` object model (`GenericRecord`, `toString`, `compare`, arrays), which is not exposed |
| io/TestValidatingIO, io/TestBlockingIO*, io/TestBlockingDirectBinaryEncoder, io/FastReaderBuilderJavaClassTest, io/TestFastReaderBuilderClassLoading | validating, blocking and class-loading encoders and decoders |
| util/TestSchemas | `Schemas.visit` (ported inside the parser's resolution; its order is covered by `parser.tsv`) |
| util/TestUtf8, util/UtfTextUtilsTest, util/TestRandomData, util/TestCaseFinder, util/NonCopyingByteArrayOutputStreamTest, util/TimePeriodTest, util/WeakIdentityHashMapTest, io/parsing/SymbolTest, TestSchemaWarnings | utility classes and logging |
