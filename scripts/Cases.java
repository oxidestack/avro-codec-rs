// Records differential vectors from Avro 1.12.2: reads `schema[TAB payload]` lines from stdin and prints each with
// what Avro did appended (see tests/vectors.rs). `<EMPTY>` stands for an empty schema text; `#` lines pass through.
import java.io.*;
import java.nio.charset.StandardCharsets;
import org.apache.avro.Schema;
import org.apache.avro.generic.*;
import org.apache.avro.io.*;

public class Cases {
  static String err(Throwable e) {
    return "ERR " + e.getClass().getSimpleName() + ": " + String.valueOf(e.getMessage()).replace("\n", "\\n");
  }

  public static void main(String[] a) throws Exception {
    BufferedReader in = new BufferedReader(new InputStreamReader(System.in, StandardCharsets.UTF_8));
    PrintStream out = new PrintStream(System.out, true, "UTF-8");
    String line;
    while ((line = in.readLine()) != null) {
      if (line.trim().isEmpty() || line.startsWith("#")) {
        out.println(line);
        continue;
      }
      String[] cols = line.split("\t", -1);
      String schemaText = cols[0].equals("<EMPTY>") ? "" : cols[0];
      String res;
      try {
        Schema s = new Schema.Parser().parse(schemaText);
        if (cols.length < 2) {
          res = "OK " + s;
        } else if (cols[1].startsWith("HEX:")) {
          String h = cols[1].substring(4);
          byte[] b = new byte[h.length() / 2];
          for (int i = 0; i < b.length; i++)
            b[i] = (byte) Integer.parseInt(h.substring(2 * i, 2 * i + 2), 16);
          try {
            res = "BIN " + toJson(s, b);
          } catch (Throwable e) {
            res = err(e);
          }
        } else {
          byte[] b;
          try {
            b = toAvro(s, cols[1]);
          } catch (Throwable e) {
            out.println(line + "\t" + err(e));
            continue;
          }
          StringBuilder hex = new StringBuilder();
          for (byte x : b)
            hex.append(String.format("%02x", x));
          try {
            res = hex + " " + toJson(s, b);
          } catch (Throwable e) {
            res = hex + " " + err(e);
          }
        }
      } catch (Throwable e) {
        res = err(e);
      }
      out.println(line + "\t" + res);
    }
  }

  // AvroPayloadCodec.jsonToAvro
  static byte[] toAvro(Schema schema, String json) throws IOException {
    GenericDatumReader<Object> reader = new GenericDatumReader<>(schema);
    Object datum = reader.read(null, DecoderFactory.get().jsonDecoder(schema, json));
    ByteArrayOutputStream out = new ByteArrayOutputStream();
    BinaryEncoder enc = EncoderFactory.get().binaryEncoder(out, null);
    new GenericDatumWriter<Object>(schema).write(datum, enc);
    enc.flush();
    return out.toByteArray();
  }

  // AvroPayloadCodec.avroToJson
  static String toJson(Schema schema, byte[] bin) throws IOException {
    GenericDatumReader<Object> reader = new GenericDatumReader<>(schema);
    Object datum = reader.read(null, DecoderFactory.get().binaryDecoder(bin, null));
    ByteArrayOutputStream out = new ByteArrayOutputStream();
    JsonEncoder enc = EncoderFactory.get().jsonEncoder(schema, out);
    new GenericDatumWriter<Object>(schema).write(datum, enc);
    enc.flush();
    return out.toString(StandardCharsets.UTF_8);
  }
}
