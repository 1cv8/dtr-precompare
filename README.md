# dtr-precompare

Обработка файлов выгрузки шины Datareon для последующего сравнения.

### Обработчики
  
  Очистка идентификаторов в полях "FolderId", "ClusterId", "EntityId"
  
  Очистка значения в поле "Version"

  Очистка координат в схемах. Поля "X" и "Y".

  Очистка значений в полях "Key" и "Id".

  Замена ID на имена в структурах "RouteSystemDataTypes", "HandlersList", "SystemMetadataId".

### Сборка

    Cargo build --release
