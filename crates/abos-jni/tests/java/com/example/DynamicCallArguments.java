package com.example;

/** Small bodies isolate JNI argument packing and validation costs. */
public class DynamicCallArguments {
    public int total;
    public static int calls;

    public static int sumStatic() {
        calls++;
        return 0;
    }

    public DynamicCallArguments() {
        total = sumStatic();
    }

    public int sum() {
        return sumStatic();
    }

    public static int sumStatic(int a0) {
        calls++;
        return a0;
    }

    public static int sumStatic(int a0, int a1) {
        calls++;
        return a0 + a1;
    }

    public static int sumStatic(int a0, int a1, int a2, int a3) {
        calls++;
        return a0 + a1 + a2 + a3;
    }

    public static int sumStatic(int a0, int a1, int a2, int a3, int a4, int a5, int a6, int a7) {
        calls++;
        return a0 + a1 + a2 + a3 + a4 + a5 + a6 + a7;
    }

    public DynamicCallArguments(int a0, int a1, int a2, int a3, int a4, int a5, int a6, int a7) {
        total = sumStatic(a0, a1, a2, a3, a4, a5, a6, a7);
    }

    public int sum(int a0, int a1, int a2, int a3, int a4, int a5, int a6, int a7) {
        return sumStatic(a0, a1, a2, a3, a4, a5, a6, a7);
    }

    public static int sumStatic(int a0, int a1, int a2, int a3, int a4, int a5, int a6, int a7, int a8) {
        calls++;
        return a0 + a1 + a2 + a3 + a4 + a5 + a6 + a7 + a8;
    }

    public DynamicCallArguments(int a0, int a1, int a2, int a3, int a4, int a5, int a6, int a7, int a8) {
        total = sumStatic(a0, a1, a2, a3, a4, a5, a6, a7, a8);
    }

    public int sum(int a0, int a1, int a2, int a3, int a4, int a5, int a6, int a7, int a8) {
        return sumStatic(a0, a1, a2, a3, a4, a5, a6, a7, a8);
    }

    public static int sumStatic(
            int a0,
            int a1,
            int a2,
            int a3,
            int a4,
            int a5,
            int a6,
            int a7,
            int a8,
            int a9,
            int a10,
            int a11,
            int a12,
            int a13,
            int a14,
            int a15) {
        calls++;
        return a0 + a1 + a2 + a3 + a4 + a5 + a6 + a7 + a8 + a9 + a10 + a11 + a12 + a13 + a14 + a15;
    }

    public static int mixed(int a0, String a1) {
        calls++;
        return a0 + (a1 == null ? 0 : a1.length());
    }

    public static int mixed(int a0, String a1, int a2, String a3, int a4, String a5, int a6, String a7) {
        calls++;
        return a0
                + (a1 == null ? 0 : a1.length())
                + a2
                + (a3 == null ? 0 : a3.length())
                + a4
                + (a5 == null ? 0 : a5.length())
                + a6
                + (a7 == null ? 0 : a7.length());
    }

    public static int mixed(int a0, String a1, int a2, String a3, int a4, String a5, int a6, String a7, int a8) {
        calls++;
        return a0
                + (a1 == null ? 0 : a1.length())
                + a2
                + (a3 == null ? 0 : a3.length())
                + a4
                + (a5 == null ? 0 : a5.length())
                + a6
                + (a7 == null ? 0 : a7.length())
                + a8;
    }

    public static int mixed(
            int a0,
            String a1,
            int a2,
            String a3,
            int a4,
            String a5,
            int a6,
            String a7,
            int a8,
            String a9,
            int a10,
            String a11,
            int a12,
            String a13,
            int a14,
            String a15) {
        calls++;
        return a0
                + (a1 == null ? 0 : a1.length())
                + a2
                + (a3 == null ? 0 : a3.length())
                + a4
                + (a5 == null ? 0 : a5.length())
                + a6
                + (a7 == null ? 0 : a7.length())
                + a8
                + (a9 == null ? 0 : a9.length())
                + a10
                + (a11 == null ? 0 : a11.length())
                + a12
                + (a13 == null ? 0 : a13.length())
                + a14
                + (a15 == null ? 0 : a15.length());
    }
}
