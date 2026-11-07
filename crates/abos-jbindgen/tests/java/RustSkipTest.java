package com.example.test;

import io.github.jni_rs.jbindgen.RustSkip;

/**
 * Test class to verify @RustSkip annotation behavior
 */
public class RustSkipTest {

    public int includedField;

    @RustSkip
    public int skippedField;

    public RustSkipTest() {}

    @RustSkip
    public RustSkipTest(int value) {}

    public void includedMethod() {}

    @RustSkip
    public void skippedMethod() {}

    public int getIncludedField() {
        return includedField;
    }

    @RustSkip
    public int getSkippedField() {
        return skippedField;
    }
}

/**
 * This entire class should be skipped
 */
@RustSkip
class SkippedClass {
    public int field;

    public void method() {}
}

/**
 * This class should be included but has some skipped members
 */
class MixedClass {
    public int normalField;

    @RustSkip
    public int internalField;

    public void publicMethod() {}

    @RustSkip
    public void internalMethod() {}
}
