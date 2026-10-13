package dev.blockcontext.client;

import java.io.*;
import java.nio.file.*;
import java.nio.file.attribute.PosixFilePermissions;
import java.util.*;
import java.security.*;

/** Bounded private factual checkpoints. No UI rows or pending claim candidates survive a restart. */
public final class CommissionLedgerFile {
    private static final int MAGIC=0x42434331,MAX_BYTES=16384;
    private static final List<String> FIELDS=List.of("schemaVersion","createdAt","claims","bonusClaims","lastAvailabilityAt","renewalAfter","renewalAt","referenceDay","dayBonusClaims","gaps","lastRemaining","lastRemainingAt");
    public static byte[] encode(Map<String,CommissionTracker> trackers)throws IOException{
        if(trackers.size()>8)throw new IOException("bounds");ByteArrayOutputStream bytes=new ByteArrayOutputStream();try(DataOutputStream out=new DataOutputStream(bytes)){
            out.writeInt(MAGIC);out.writeInt(trackers.size());for(var entry:trackers.entrySet()){
                if(!key(entry.getKey()))throw new IOException("key");out.writeUTF(entry.getKey());var s=entry.getValue().persistentState();for(String f:FIELDS)out.writeLong(((Number)s.get(f)).longValue());Object a=s.get("lastAvailability");out.writeByte(a==null?-1:Boolean.TRUE.equals(a)?1:0);
            }
        }byte[] payload=bytes.toByteArray();bytes.write(digest(payload));return bytes.toByteArray();
    }
    public static Map<String,CommissionTracker> decode(byte[] bytes,long now)throws IOException{
        if(bytes.length>MAX_BYTES||bytes.length<40)throw new IOException("bounds");byte[] payload=Arrays.copyOf(bytes,bytes.length-32);if(!MessageDigest.isEqual(digest(payload),Arrays.copyOfRange(bytes,bytes.length-32,bytes.length)))throw new IOException("checksum");Map<String,CommissionTracker> result=new LinkedHashMap<>();try(DataInputStream in=new DataInputStream(new ByteArrayInputStream(payload))){
            if(in.readInt()!=MAGIC)throw new IOException("schema");int count=in.readInt();if(count<0||count>8)throw new IOException("bounds");for(int i=0;i<count;i++){
                String key=in.readUTF();if(!key(key)||result.containsKey(key))throw new IOException("key");Map<String,Object> s=new LinkedHashMap<>();for(String f:FIELDS)s.put(f,in.readLong());int a=in.readByte();if(a<-1||a>1)throw new IOException("availability");s.put("lastAvailability",a==-1?null:a==1);
                try{result.put(key,CommissionTracker.restore(s,now));}catch(IllegalArgumentException e){throw new IOException("invalid checkpoint");}
            }if(in.available()!=0)throw new IOException("trailing bytes");
        }return result;
    }
    private static boolean key(String value){return value.matches("[a-f0-9]{32}:[a-z][a-z ]{0,31}");}
    private static byte[] digest(byte[] bytes)throws IOException{try{return MessageDigest.getInstance("SHA-256").digest(bytes);}catch(NoSuchAlgorithmException e){throw new IOException("checksum unavailable");}}
    public static Map<String,CommissionTracker> read(Path file,long now)throws IOException{
        if(!Files.exists(file,LinkOption.NOFOLLOW_LINKS))return new LinkedHashMap<>();check(file);
        if(!Files.isRegularFile(file,LinkOption.NOFOLLOW_LINKS)||Files.size(file)>MAX_BYTES)throw new IOException("bounds");return decode(Files.readAllBytes(file),now);
    }
    public static void write(Path file,byte[] bytes)throws IOException{
        if(bytes.length>MAX_BYTES)throw new IOException("bounds");Files.createDirectories(file.getParent());check(file.getParent());if(Files.isSymbolicLink(file))throw new IOException("symlink");
        boolean posix=Files.getFileStore(file.getParent()).supportsFileAttributeView("posix");Path temp=posix?Files.createTempFile(file.getParent(),".commission-ledger-",".tmp",PosixFilePermissions.asFileAttribute(PosixFilePermissions.fromString("rw-------"))):Files.createTempFile(file.getParent(),".commission-ledger-",".tmp");
        try{Files.write(temp,bytes,StandardOpenOption.TRUNCATE_EXISTING);Files.move(temp,file,StandardCopyOption.REPLACE_EXISTING,StandardCopyOption.ATOMIC_MOVE);}finally{Files.deleteIfExists(temp);}
    }
    private static void check(Path file)throws IOException{if(Files.isSymbolicLink(file)||Files.isSymbolicLink(file.getParent()))throw new IOException("symlink");if(Files.getFileStore(file).supportsFileAttributeView("posix")&&Files.getPosixFilePermissions(file).stream().anyMatch(p->p.name().startsWith("GROUP_")||p.name().startsWith("OTHERS_")))throw new IOException("private checkpoint required");}
}
