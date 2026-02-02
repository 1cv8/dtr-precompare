# dtr-precompare

Обработка файлов выгрузки шины Datareon для последующего сравнения.

### Обработчики
  
  Очистка идентификаторов в полях "FolderId", "ClusterId", "EntityId"
  
  Очистка значения в поле "Version"

  Очистка координат в схемах. Поля "X" и "Y".

### Альтернатива

```shell
#!/bin/sh
find . -type f -name '*.json' -exec sed -i \
  -e 's/"FolderId":[[:space:]]*"[0-9a-f-]*"/"FolderId":"00000000-0000-0000-0000-000000000000"/g' \
  -e 's/"ClusterId":[[:space:]]*"[0-9a-f-]*"/"ClusterId":"00000000-0000-0000-0000-000000000000"/g' \
  -e 's/"EntityId":[[:space:]]*"[0-9a-f-]*"/"EntityId":"00000000-0000-0000-0000-000000000000"/g' \
  -e 's/"Version":[[:space:]]*[0-9]\+,[[:space:]]*/"Version":0,/g' \
  -e 's/"X":[[:space:]]*[0-9]\+/"X":0/g' \
  -e 's/"Y":[[:space:]]*[0-9]\+/"Y":0/g' \
  {} \;
```

### Сборка

    Cargo build --release
