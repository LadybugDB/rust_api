#pragma once

#include "rust/cxx.h"
#ifdef LBUG_BUNDLED
#include "main/lbug.h"
#include "storage/table/arrow_table_support.h"
#else
#include <lbug.hpp>
#endif

namespace lbug_arrow {

ArrowSchema query_result_get_arrow_schema(const lbug::main::QueryResult& result);
ArrowArray query_result_get_next_arrow_chunk(lbug::main::QueryResult& result, uint64_t chunkSize);

// Zero-copy Arrow import: create a node table backed by in-memory Arrow data.
// The schema and array are moved into the registry — Rust transfers ownership.
// Returns the arrow registry ID (needed to later drop/unregister the table).
rust::String create_node_table_from_arrow(lbug::main::Connection& connection,
    rust::Str table_name, ArrowSchema schema, ArrowArray array);

// Unregister (drop) an arrow-backed table created by create_node_table_from_arrow.
void drop_arrow_table(lbug::main::Connection& connection, rust::Str table_name);

} // namespace lbug_arrow
