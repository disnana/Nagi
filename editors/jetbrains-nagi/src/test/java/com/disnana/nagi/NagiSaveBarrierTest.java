package com.disnana.nagi;

import com.intellij.openapi.progress.ProcessCanceledException;
import java.io.IOException;
import org.junit.Test;
import static org.junit.Assert.*;

public class NagiSaveBarrierTest {
    public static final class Absent {}
    public static final class Success {
        boolean invoked;
        public void flushPendingUpdates() { invoked = true; }
    }
    public static final class FailsWithIo {
        public void flushPendingUpdates() throws IOException { throw new IOException("disk write failed"); }
    }
    public static final class Cancelled {
        public void flushPendingUpdates() { throw new ProcessCanceledException(); }
    }
    public static final class WrongSignature {
        public int flushPendingUpdates() { return 1; }
    }

    @Test public void knownSynchronousBaselinesCanOmitTheBarrier() throws Exception {
        for (int baseline : new int[] {251, 252, 253, 261}) {
            NagiSaveBarrier.flush(Absent.class, new Absent(), baseline);
        }
    }

    @Test public void missingBarrierFailsClosedOnAsyncAndUnknownBaselines() {
        for (int baseline : new int[] {254, 262, 263, 264}) {
            assertThrows(IOException.class, () -> NagiSaveBarrier.flush(Absent.class, new Absent(), baseline));
        }
    }

    @Test public void presentBarrierIsInvokedAndErrorsDoNotBecomeSuccess() throws Exception {
        var success = new Success();
        NagiSaveBarrier.flush(Success.class, success, 263);
        assertTrue(success.invoked);
        var io = assertThrows(IOException.class,
                () -> NagiSaveBarrier.flush(FailsWithIo.class, new FailsWithIo(), 263));
        assertEquals("disk write failed", io.getMessage());
        assertThrows(IOException.class,
                () -> NagiSaveBarrier.flush(WrongSignature.class, new WrongSignature(), 263));
        var denied = assertThrows(IOException.class,
                () -> NagiSaveBarrier.flush(Success.class, success, 263,
                        (method, target) -> { throw new IllegalAccessException("reflective access denied"); }));
        assertTrue(denied.getCause() instanceof IllegalAccessException);
    }

    @Test public void cancellationRemainsCancellation() {
        assertThrows(ProcessCanceledException.class,
                () -> NagiSaveBarrier.flush(Cancelled.class, new Cancelled(), 263));
    }
}
