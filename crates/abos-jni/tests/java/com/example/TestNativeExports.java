package com.example;

public class TestNativeExports {
    public TestNativeExports() {}

    public native int noArgs();

    public native int primitiveArgs(int a, long b, boolean c);

    public native void primitiveArrayArgs(int[] arr, byte[] bytes);

    public native String objectArgs(String str, Object obj);

    public native void objectArrayArgs(String[] strings, Object[] objects);

    public native int overloaded();

    public native int overloaded(int x);

    public native int overloaded(String s);

    // Test: unicode in method name (needs mangling)
    public native void méthod();

    // Test: underscore in method name (needs mangling)
    public native void method_with_underscore();

    // Test: manual export symbol
    public native void manualExport();

    // Inner class for testing '$' in class name
    public static class Inner {
        public native void innerMethod();
    }
}
