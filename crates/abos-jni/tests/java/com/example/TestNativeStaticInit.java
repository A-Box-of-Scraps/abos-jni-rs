package com.example;

public class TestNativeStaticInit {
    private static int staticValue = 5;

    static {
        staticValue += 10;
        System.out.println("Static initializer starting...");
        staticValue = nativeInitializeStatic(staticValue);
        System.out.println("Static initializer completed. Value: " + staticValue);
    }

    private static native int nativeInitializeStatic(int counter);

    public static int getStaticValue() {
        return staticValue;
    }
}
