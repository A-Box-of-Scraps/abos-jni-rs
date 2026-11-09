package com.example;

public class CallableModifiers {
    /** Documentation from the non-native stub. */
    @Deprecated
    public int nativeValue() {
        throw new RuntimeException("Stub!");
    }

    public static int staticNativeValue() {
        throw new RuntimeException("Stub!");
    }
    /** Documentation from the native stub. */
    public native int ordinaryValue(int documentedName);

    public static native int staticOrdinaryValue();

    public native int matchedNative();

    public static native int matchedStaticNative();

    public int matchedOrdinary() {
        throw new RuntimeException("Stub!");
    }

    public static int matchedStaticOrdinary() {
        throw new RuntimeException("Stub!");
    }

    public int overloaded(int value) {
        throw new RuntimeException("Stub!");
    }

    public int overloaded(String value) {
        throw new RuntimeException("Stub!");
    }

    public int overloaded(long value) {
        throw new RuntimeException("Stub!");
    }

    public int wrongParameter(long value) {
        throw new RuntimeException("Stub!");
    }

    public int wrongArray(int[][] value) {
        throw new RuntimeException("Stub!");
    }

    public long wrongReturn() {
        throw new RuntimeException("Stub!");
    }

    public int wrongStatic() {
        throw new RuntimeException("Stub!");
    }

    public static native int wrongInstance();

    public int stubOnly() {
        throw new RuntimeException("Stub!");
    }
}
