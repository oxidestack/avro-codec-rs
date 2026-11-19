#!/bin/sh
# Re-records a vector file from Avro 1.12.2: scripts/record.sh tests/vectors/<file>.tsv
# Keeps each row's schema and payload columns and asks Avro again. Needs a JDK and the jars in ~/.m2.
set -eu
M="$HOME/.m2/repository"
CP="$M/org/apache/avro/avro/1.12.2/avro-1.12.2.jar:$M/com/fasterxml/jackson/core/jackson-core/2.22.2/jackson-core-2.22.2.jar:$M/com/fasterxml/jackson/core/jackson-databind/2.22.2/jackson-databind-2.22.2.jar:$M/com/fasterxml/jackson/core/jackson-annotations/2.22/jackson-annotations-2.22.jar:$M/org/slf4j/slf4j-api/2.0.17/slf4j-api-2.0.17.jar:$M/org/apache/commons/commons-compress/1.28.0/commons-compress-1.28.0.jar"
DIR=$(mktemp -d)
javac -d "$DIR" -cp "$CP" "$(dirname "$0")/Cases.java"
# A row is `schema TAB result` or `schema TAB payload TAB result`: drop the result.
awk -F'\t' 'BEGIN{OFS="\t"} /^#/ || /^[[:space:]]*$/ {print; next} NF==3 {print $1,$2; next} NF==2 && $2 ~ /^(OK |ERR |BIN )/ {print $1; next} {print}' "$1" \
  | java -cp "$CP:$DIR" Cases 2>/dev/null > "$1.new"
mv "$1.new" "$1"
rm -rf "$DIR"
