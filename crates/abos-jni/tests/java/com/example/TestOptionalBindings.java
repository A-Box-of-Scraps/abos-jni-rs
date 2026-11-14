package com.example;

public class TestOptionalBindings {
    public int value = 7;
    public Object object;
    public static int staticValue = requiredNative(10) + optionalNative(20) + guardedNative(3);
    public static Object staticObject;

    public TestOptionalBindings() {}

    public TestOptionalBindings(int value) {
        this.value = value;
    }

    public int present() {
        return value;
    }

    public static int staticPresent() {
        return staticValue;
    }

    public static boolean disabledNativeIsUnregistered() {
        try {
            disabledNative(0);
            return false;
        } catch (UnsatisfiedLinkError expected) {
            return true;
        }
    }

    public native int instanceNative(int value);

    private static native int requiredNative(int value);

    private static native int optionalNative(int value);

    private static native int guardedNative(int value);

    private static native int disabledNative(int value);
}
