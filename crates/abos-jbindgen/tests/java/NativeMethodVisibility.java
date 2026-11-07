package com.example;

/**
 * Test class with both public and private native methods for testing visibility
 * handling.
 */
public class NativeMethodVisibility {
    public static native int publicStaticNative(int a, int b);

    public native String publicInstanceNative();

    private static native int privateStaticNative(int x);

    private native void privateInstanceNative(String data);

    public int regularMethod(int x) {
        return privateStaticNative(x) * 2;
    }

    static {
        // System.loadLibrary("nativemethodvisibility");
    }
}
