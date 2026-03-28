#include "lbug_arrow.h"

namespace lbug_arrow {

ArrowSchema query_result_get_arrow_schema(const lbug::main::QueryResult& result) {
    // Could use directly, except that we can't (yet) mark ArrowSchema as being safe to store in a
    // cxx::UniquePtr
    return *result.getArrowSchema();
}

ArrowArray query_result_get_next_arrow_chunk(lbug::main::QueryResult& result, uint64_t chunkSize) {
    return *result.getNextArrowChunk(chunkSize);
}

rust::String create_node_table_from_arrow(lbug::main::Connection& connection,
    rust::Str table_name, ArrowSchema schema, ArrowArray array) {
    // Move the raw C structs into the RAII wrappers. This transfers ownership:
    // the wrappers will call the release callbacks when they're destroyed.
    ArrowSchemaWrapper schemaWrapper;
    static_cast<ArrowSchema&>(schemaWrapper) = schema;
    // Prevent Rust/cxx from calling release — the wrapper owns it now.
    schema.release = nullptr;

    ArrowArrayWrapper arrayWrapper;
    static_cast<ArrowArray&>(arrayWrapper) = array;
    array.release = nullptr;

    // ArrowTableSupport expects a vector of arrays (one per batch).
    std::vector<ArrowArrayWrapper> arrays;
    arrays.push_back(std::move(arrayWrapper));

    std::string name(table_name.data(), table_name.size());
    auto result = lbug::ArrowTableSupport::createViewFromArrowTable(
        connection, name, std::move(schemaWrapper), std::move(arrays));

    if (!result.queryResult->isSuccess()) {
        throw std::runtime_error(result.queryResult->getErrorMessage());
    }

    return rust::String(result.arrowId);
}

void drop_arrow_table(lbug::main::Connection& connection, rust::Str table_name) {
    std::string name(table_name.data(), table_name.size());
    auto result = lbug::ArrowTableSupport::unregisterArrowTable(connection, name);
    if (!result->isSuccess()) {
        throw std::runtime_error(result->getErrorMessage());
    }
}

} // namespace lbug_arrow
