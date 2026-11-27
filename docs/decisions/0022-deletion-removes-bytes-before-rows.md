## 0022 Deletion removes bytes before rows

Accepted. Unpublish, delete blobs under the tenant, delete rows, send a receipt. A crash leaves a
diagnosable row, never orphaned bytes.
