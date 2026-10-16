package dev.blockcontext.client;
import java.io.*;
import java.nio.file.*;
import java.security.*;
import java.util.*;
/** Small private checkpoints; no menu rows, reward contents or event history. */
public final class ExperimentLedgerFile {
    private static byte[] digest(byte[] bytes)throws IOException{try{return MessageDigest.getInstance("SHA-256").digest(bytes);}catch(NoSuchAlgorithmException e){throw new IOException("checksum unavailable");}}
    public static byte[] encode(Map<String,ExperimentTracker> trackers)throws IOException{
        if(trackers.size()>8)throw new IOException("bounds");ByteArrayOutputStream bytes=new ByteArrayOutputStream();
        try(DataOutputStream out=new DataOutputStream(bytes)){out.writeInt(0x42434531);out.writeInt(trackers.size());for(var e:trackers.entrySet()){
            if(!e.getKey().matches("[a-f0-9]{32}:[a-z][a-z ]{0,31}"))throw new IOException("key");out.writeUTF(e.getKey());for(long n:e.getValue().checkpoint())out.writeLong(n);
        }}byte[] payload=bytes.toByteArray();bytes.write(digest(payload));return bytes.toByteArray();
    }
    public static Map<String,ExperimentTracker> decode(byte[] bytes,long now)throws IOException{
        if(bytes.length<40||bytes.length>16384)throw new IOException("bounds");byte[] payload=Arrays.copyOf(bytes,bytes.length-32);if(!MessageDigest.isEqual(digest(payload),Arrays.copyOfRange(bytes,bytes.length-32,bytes.length)))throw new IOException("checksum");
        Map<String,ExperimentTracker> out=new LinkedHashMap<>();try(DataInputStream in=new DataInputStream(new ByteArrayInputStream(payload))){
            if(in.readInt()!=0x42434531)throw new IOException("schema");int count=in.readInt();if(count<0||count>8)throw new IOException("bounds");for(int i=0;i<count;i++){
                String key=in.readUTF();if(!key.matches("[a-f0-9]{32}:[a-z][a-z ]{0,31}")||out.containsKey(key))throw new IOException("key");long[] s=new long[6];for(int j=0;j<6;j++)s[j]=in.readLong();
                try{out.put(key,ExperimentTracker.restore(s,now));}catch(IllegalArgumentException e){throw new IOException("checkpoint");}
            }if(in.available()!=0)throw new IOException("trailing bytes");
        }return out;
    }
    public static Map<String,ExperimentTracker> read(Path file,long now)throws IOException{
        if(!Files.exists(file,LinkOption.NOFOLLOW_LINKS))return new LinkedHashMap<>();
        if(Files.isSymbolicLink(file)||Files.isSymbolicLink(file.getParent())||!Files.isRegularFile(file,LinkOption.NOFOLLOW_LINKS)||Files.size(file)>16384)throw new IOException("private checkpoint required");
        if(Files.getFileStore(file).supportsFileAttributeView("posix")&&Files.getPosixFilePermissions(file).stream().anyMatch(p->p.name().startsWith("GROUP_")||p.name().startsWith("OTHERS_")))throw new IOException("private checkpoint required");return decode(Files.readAllBytes(file),now);
    }
}
