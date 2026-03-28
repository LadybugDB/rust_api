use crate::database::Database;
use crate::error::Error;
use crate::ffi::ffi;
use crate::query_result::QueryResult;
use crate::value::Value;
use cxx::UniquePtr;
use std::cell::UnsafeCell;
use std::convert::TryInto;

/// A prepared stattement is a parameterized query which can avoid planning the same query for
/// repeated execution
pub struct PreparedStatement {
    statement: UniquePtr<ffi::PreparedStatement>,
}

/// Connections are used to interact with a Database instance.
///
/// ## Concurrency
///
/// Each connection is thread-safe, and multiple connections can connect to the same Database
/// instance in a multithreaded environment.
///
/// Note that since connections require a reference to the Database, creating or using connections
/// in multiple threads cannot be done from a regular `std::thread` since the threads (and
/// connections) could outlive the database. This can be worked around by using a
/// [scoped thread](std::thread::scope) (Note: Introduced in rust 1.63. For compatibility with
/// older versions of rust, [crosssbeam_utils::thread::scope](https://docs.rs/crossbeam-utils/latest/crossbeam_utils/thread/index.html) can be used instead).
///
/// Also note that write queries can only be done one at a time; the query command will return an
/// [error](Error::FailedQuery) if another write query is in progress.
///
/// ```
/// # use lbug::{Connection, Database, SystemConfig, Value, Error};
/// # fn main() -> anyhow::Result<()> {
/// # let temp_dir = tempfile::tempdir()?;
/// # let db = Database::new(temp_dir.path().join("testdb"), SystemConfig::default())?;
/// let conn = Connection::new(&db)?;
/// conn.query("CREATE NODE TABLE Person(name STRING, age INT32, PRIMARY KEY(name));")?;
/// // Write queries must be done sequentially
/// conn.query("CREATE (:Person {name: 'Alice', age: 25});")?;
/// conn.query("CREATE (:Person {name: 'Bob', age: 30});")?;
/// let (alice, bob) = std::thread::scope(|s| -> Result<(Vec<Value>, Vec<Value>), Error> {
///     let alice_thread = s.spawn(|| -> Result<Vec<Value>, Error> {
///         let conn = Connection::new(&db)?;
///         let mut result = conn.query("MATCH (a:Person) WHERE a.name = \"Alice\" RETURN a.name AS NAME, a.age AS AGE;")?;
///         Ok(result.next().unwrap())
///     });
///     let bob_thread = s.spawn(|| -> Result<Vec<Value>, Error> {
///         let conn = Connection::new(&db)?;
///         let mut result = conn.query(
///             "MATCH (a:Person) WHERE a.name = \"Bob\" RETURN a.name AS NAME, a.age AS AGE;",
///         )?;
///         Ok(result.next().unwrap())
///     });
///     Ok((alice_thread.join().unwrap()?, bob_thread.join().unwrap()?))
///  })?;
///
///  assert_eq!(alice, vec!["Alice".into(), 25.into()]);
///  assert_eq!(bob, vec!["Bob".into(), 30.into()]);
///  temp_dir.close()?;
///  Ok(())
/// # }
/// ```
///
pub struct Connection<'a> {
    // bmwinger: Access to the underlying value for synchronized functions can be done
    // with (*self.conn.get()).pin_mut()
    // Turning this into a function just causes lifetime issues.
    conn: UnsafeCell<UniquePtr<ffi::Connection<'a>>>,
}

// Connections are synchronized on the C++ side and should be safe to move and access across
// threads
unsafe impl Send for Connection<'_> {}
unsafe impl Sync for Connection<'_> {}

impl<'a> Connection<'a> {
    /// Creates a connection to the database.
    ///
    /// # Arguments
    /// * `database`: A reference to the database instance to which this connection will be connected.
    pub fn new(database: &'a Database) -> Result<Self, Error> {
        let db = unsafe { (*database.db.get()).pin_mut() };
        Ok(Connection {
            conn: UnsafeCell::new(ffi::database_connect(db)?),
        })
    }

    /// Sets the maximum number of threads to use for execution in the current connection
    ///
    /// # Arguments
    /// * `num_threads`: The maximum number of threads to use for execution in the current connection
    pub fn set_max_num_threads_for_exec(&mut self, num_threads: u64) {
        self.conn
            .get_mut()
            .pin_mut()
            .setMaxNumThreadForExec(num_threads);
    }

    /// Returns the maximum number of threads used for execution in the current connection
    pub fn get_max_num_threads_for_exec(&self) -> u64 {
        unsafe { (*self.conn.get()).pin_mut().getMaxNumThreadForExec() }
    }

    /// Prepares the given query and returns the prepared statement. [`PreparedStatement`]s can be run
    /// using [`Connection::execute`]
    ///
    /// # Arguments
    /// * `query`: The query to prepare. See <https://ladybugdb.com/docs/cypher> for details on the
    ///   query format.
    pub fn prepare(&self, query: &str) -> Result<PreparedStatement, Error> {
        let statement =
            unsafe { (*self.conn.get()).pin_mut() }.prepare(ffi::StringView::new(query))?;
        if statement.isSuccess() {
            Ok(PreparedStatement { statement })
        } else {
            Err(Error::FailedPreparedStatement(
                ffi::prepared_statement_error_message(&statement),
            ))
        }
    }

    /// Executes the given query and returns the result.
    ///
    /// # Arguments
    /// * `query`: The query to execute. See <https://ladybugdb.com/docs/cypher> for details on the
    ///   query format.
    pub fn query(&self, query: &str) -> Result<QueryResult<'a>, Error> {
        let conn = unsafe { (*self.conn.get()).pin_mut() };
        let result = ffi::connection_query(conn, ffi::StringView::new(query))?;
        if result.isSuccess() {
            Ok(QueryResult { result })
        } else {
            Err(Error::FailedQuery(ffi::query_result_get_error_message(
                &result,
            )))
        }
    }

    /// Executes the given prepared statement with args and returns the result.
    ///
    /// # Arguments
    /// * `prepared_statement`: The prepared statement to execute
    ///```
    /// # use lbug::{Database, SystemConfig, Connection, Value};
    /// # use anyhow::Error;
    /// #
    /// # fn main() -> Result<(), Error> {
    /// # let temp_dir = tempfile::tempdir()?;
    /// # let path = temp_dir.path().join("testdb");
    /// # let db = Database::new(path, SystemConfig::default())?;
    /// let conn = Connection::new(&db)?;
    /// conn.query("CREATE NODE TABLE Person(name STRING, age INT64, PRIMARY KEY(name));")?;
    /// let mut prepared = conn.prepare("CREATE (:Person {name: $name, age: $age});")?;
    /// conn.execute(&mut prepared,
    ///     vec![("name", Value::String("Alice".to_string())), ("age", Value::Int64(25))])?;
    /// conn.execute(&mut prepared,
    ///     vec![("name", Value::String("Bob".to_string())), ("age", Value::Int64(30))])?;
    /// # temp_dir.close()?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn execute(
        &self,
        prepared_statement: &mut PreparedStatement,
        params: Vec<(&str, Value)>,
    ) -> Result<QueryResult<'a>, Error> {
        let mut cxx_params = ffi::new_params();
        for (key, value) in params {
            let ffi_value: cxx::UniquePtr<ffi::Value> = value.try_into()?;
            cxx_params.pin_mut().insert(key, ffi_value);
        }
        let conn = unsafe { (*self.conn.get()).pin_mut() };
        let result =
            ffi::connection_execute(conn, prepared_statement.statement.pin_mut(), cxx_params)?;
        if result.isSuccess() {
            Ok(QueryResult { result })
        } else {
            Err(Error::FailedQuery(ffi::query_result_get_error_message(
                &result,
            )))
        }
    }

    /// Create a node table backed by an in-memory Arrow `RecordBatch` -- **zero-copy**.
    ///
    /// The data is registered in LadybugDB's Arrow registry and the table is created
    /// with `storage='arrow://...'`. The table can then be queried with Cypher just
    /// like any other table. The Arrow data stays in memory (owned by the registry)
    /// until the table is dropped via [`Connection::drop_arrow_table`].
    ///
    /// The first column of the `RecordBatch` is used as the primary key.
    ///
    /// Returns the arrow registry ID (needed for lifecycle management).
    ///
    /// *Requires the `arrow` feature.*
    #[cfg(feature = "arrow")]
    pub fn create_node_table_from_arrow(
        &self,
        table_name: &str,
        batch: &arrow::record_batch::RecordBatch,
    ) -> Result<String, Error> {
        use arrow::array::Array;

        let struct_array: arrow::array::StructArray = batch.clone().into();
        let array_data = struct_array.into_data();

        let (ffi_array, ffi_schema) = arrow::ffi::to_ffi(&array_data)
            .map_err(|e| Error::ArrowError(e))?;

        let arrow_array = crate::ffi::arrow::ArrowArray(ffi_array);
        let arrow_schema = crate::ffi::arrow::ArrowSchema(ffi_schema);

        let conn = unsafe { (*self.conn.get()).pin_mut() };
        let arrow_id = crate::ffi::arrow::ffi_arrow::create_node_table_from_arrow(
            conn,
            table_name,
            arrow_schema,
            arrow_array,
        )?;

        Ok(arrow_id)
    }

    /// Insert rows from an Arrow `RecordBatch` into an **existing** node table.
    ///
    /// Creates a temporary Arrow-backed table, copies the data into the target
    /// table via `COPY ... FROM (MATCH ...)`, then drops the temporary table.
    ///
    /// *Requires the `arrow` feature.*
    #[cfg(feature = "arrow")]
    pub fn insert_arrow(
        &self,
        table_name: &str,
        batch: &arrow::record_batch::RecordBatch,
    ) -> Result<QueryResult<'a>, Error> {
        let temp_name = format!("_arrow_tmp_{}", table_name);
        self.create_node_table_from_arrow(&temp_name, batch)?;

        let columns: Vec<String> = batch
            .schema()
            .fields()
            .iter()
            .map(|f| format!("t.{}", f.name()))
            .collect();
        let col_list = columns.join(", ");

        let copy_query = format!(
            "COPY {table_name} FROM (MATCH (t:{temp_name}) RETURN {col_list})"
        );
        let result = self.query(&copy_query);
        let _ = self.drop_arrow_table(&temp_name);
        result
    }

    /// Upsert rows from an Arrow `RecordBatch` into an existing node table.
    ///
    /// Uses Cypher `MERGE` to match on the primary key (first column).
    /// Existing rows get updated; new rows get created.
    ///
    /// *Requires the `arrow` feature.*
    #[cfg(feature = "arrow")]
    pub fn upsert_arrow(
        &self,
        table_name: &str,
        batch: &arrow::record_batch::RecordBatch,
    ) -> Result<QueryResult<'a>, Error> {
        let schema = batch.schema();
        let fields: Vec<&str> = schema
            .fields()
            .iter()
            .map(|f| f.name().as_str())
            .collect();

        if fields.is_empty() {
            return Err(Error::FailedQuery("RecordBatch has no columns".into()));
        }

        let temp_name = format!("_arrow_tmp_{}", table_name);
        self.create_node_table_from_arrow(&temp_name, batch)?;

        let pk = fields[0];
        let non_key_fields: Vec<&&str> = fields[1..].iter().collect();

        let set_clause: String = non_key_fields
            .iter()
            .map(|f| format!("n.{f} = t.{f}"))
            .collect::<Vec<_>>()
            .join(", ");

        let query = if non_key_fields.is_empty() {
            format!(
                "MATCH (t:{temp_name}) MERGE (n:{table_name} {{{pk}: t.{pk}}})"
            )
        } else {
            format!(
                "MATCH (t:{temp_name}) \
                 MERGE (n:{table_name} {{{pk}: t.{pk}}}) \
                 ON MATCH SET {set_clause} \
                 ON CREATE SET {set_clause}"
            )
        };

        let result = self.query(&query);
        let _ = self.drop_arrow_table(&temp_name);
        result
    }

    /// Drop an Arrow-backed table and release its in-memory data.
    ///
    /// *Requires the `arrow` feature.*
    #[cfg(feature = "arrow")]
    pub fn drop_arrow_table(&self, table_name: &str) -> Result<(), Error> {
        let conn = unsafe { (*self.conn.get()).pin_mut() };
        Ok(crate::ffi::arrow::ffi_arrow::drop_arrow_table(conn, table_name)?)
    }

    /// Interrupts all queries currently executing within this connection
    pub fn interrupt(&self) -> Result<(), Error> {
        let conn = unsafe { (*self.conn.get()).pin_mut() };
        Ok(conn.interrupt()?)
    }

    /// Sets the query timeout value of the current connection
    ///
    /// A value of zero (the default) disables the timeout.
    pub fn set_query_timeout(&self, timeout_ms: u64) {
        let conn = unsafe { (*self.conn.get()).pin_mut() };
        conn.setQueryTimeOut(timeout_ms);
    }
}
