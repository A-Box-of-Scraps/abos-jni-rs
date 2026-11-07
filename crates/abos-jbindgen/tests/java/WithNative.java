package com.example;

/**
 * Test class with native methods for jbindgen.
 */
public class WithNative {
    public static native int nativeAdd(int a, int b);

    public native String nativeGetMessage();

    public native void nativeProcess(String input, double value);

    public static native String nativeGetPlatform();

    public int regularMethod(int x) {
        return x * 2;
    }

    static {
        // System.loadLibrary("withnative");
    }
}
