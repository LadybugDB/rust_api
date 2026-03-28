#[repr(transparent)]
pub struct ArrowArray(pub arrow::ffi::FFI_ArrowArray);

#[repr(transparent)]
pub struct ArrowSchema(pub arrow::ffi::FFI_ArrowSchema);

unsafe impl cxx::ExternType for ArrowArray {
    type Id = cxx::type_id!("ArrowArray");
    type Kind = cxx::kind::Trivial;
}

unsafe impl cxx::ExternType for ArrowSchema {
    type Id = cxx::type_id!("ArrowSchema");
    type Kind = cxx::kind::Trivial;
}

#[cxx::bridge]
pub(crate) mod ffi_arrow {
    unsafe extern "C++" {
        include!("lbug/include/lbug_arrow.h");

        #[namespace = "lbug::main"]
        type QueryResult<'db> = crate::ffi::ffi::QueryResult<'db>;

        #[namespace = "lbug::main"]
        type Connection<'db> = crate::ffi::ffi::Connection<'db>;
    }

    unsafe extern "C++" {
        type ArrowArray = crate::ffi::arrow::ArrowArray;

        #[namespace = "lbug_arrow"]
        fn query_result_get_next_arrow_chunk<'db>(
            result: Pin<&mut QueryResult<'db>>,
            chunk_size: u64,
        ) -> Result<ArrowArray>;
    }

    unsafe extern "C++" {
        type ArrowSchema = crate::ffi::arrow::ArrowSchema;

        #[namespace = "lbug_arrow"]
        fn query_result_get_arrow_schema<'db>(result: &QueryResult<'db>) -> Result<ArrowSchema>;
    }

    // Zero-copy Arrow import
    unsafe extern "C++" {
        #[namespace = "lbug_arrow"]
        fn create_node_table_from_arrow<'db>(
            connection: Pin<&mut Connection<'db>>,
            table_name: &str,
            schema: ArrowSchema,
            array: ArrowArray,
        ) -> Result<String>;

        #[namespace = "lbug_arrow"]
        fn drop_arrow_table<'db>(
            connection: Pin<&mut Connection<'db>>,
            table_name: &str,
        ) -> Result<()>;
    }
}
