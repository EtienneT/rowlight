# Try Tablelamp with sample data

[Download demo.parquet](https://etiennet.github.io/tablelamp/demo/demo.parquet)
· [SHA-256 checksum](https://etiennet.github.io/tablelamp/demo/demo.parquet.sha256)
· [Install Tablelamp](../installation.md)

This 10 KB file contains 1,000 entirely synthetic rows. It includes identifiers,
amounts, booleans, regions, notes and dates, with deliberate nulls. It contains no
personal data, credentials, machine paths or cloud URLs and is MIT licensed.

Open the file in Tablelamp, expand the **amount** chart, inspect a **note** cell,
and try paging with 500 rows per page. Charts describe the displayed page.

Choose **SQL query…** from the app menu and run:

```sql
SELECT region, COUNT(*) AS rows
FROM data
GROUP BY region
ORDER BY region
```

The complete file has these counts:

| region | rows |
| --- | ---: |
| Central | 154 |
| East | 231 |
| North | 231 |
| South | 230 |
| West | 154 |

The [sample query](demo.sql) also computes average amounts for active rows.
SQL uses the complete opened source rather than just the visible page.

The file comes from the first 1,000 rows of the deterministic
[sample generator](../../examples/sample.rs). To regenerate it at a new path:

```sh
cargo run --locked --features cloud --example sample -- /tmp/tablelamp-demo.parquet 1000
```

The generator refuses to overwrite existing files.
