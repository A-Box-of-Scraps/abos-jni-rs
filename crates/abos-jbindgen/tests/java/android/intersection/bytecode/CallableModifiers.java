package com.example;

public class CallableModifiers {
    public native int nativeValue();

    public static native int staticNativeValue();

    public int ordinaryValue(int bytecodeName) {
        return bytecodeName;
    }

    public static int staticOrdinaryValue() {
        return 0;
    }

    public native int matchedNative();

    public static native int matchedStaticNative();

    public int matchedOrdinary() {
        return 0;
    }

    public static int matchedStaticOrdinary() {
        return 0;
    }

    public native int overloaded(int value);

    public native int overloaded(String value);

    public native int wrongParameter(int value);

    public native int wrongArray(int[] value);

    public native int wrongReturn();

    public static native int wrongStatic();

    public int wrongInstance() {
        return 0;
    }

    public native int jarOnly();
}
