package com.jbindgen;

import com.sun.source.tree.*;
import com.sun.source.util.*;
import java.io.*;
import java.net.*;
import java.nio.charset.StandardCharsets;
import java.nio.file.*;
import java.util.*;
import java.util.jar.*;
import javax.lang.model.element.*;
import javax.lang.model.type.*;
import javax.lang.model.util.*;
import javax.tools.*;

/**
 * Java source parser that uses JavacTask to analyze Java source files
 * and extract API information including fully-qualified types and parameter
 * names.
 *
 * Supports reading source files from:
 * - Regular filesystem .java files
 * - Source files within JAR archives
 */
public class Parser implements AutoCloseable {
    private final JavaCompiler compiler;
    private final StandardJavaFileManager fileManager;

    public Parser() {
        this.compiler = ToolProvider.getSystemJavaCompiler();
        if (this.compiler == null) {
            throw new RuntimeException("No Java compiler available. Please ensure you're running with a JDK.");
        }
        this.fileManager = compiler.getStandardFileManager(null, null, null);
    }

    @Override
    public void close() throws IOException {
        fileManager.close();
    }

    /**
     * Custom JavaFileObject for reading source files from JAR archives.
     */
    private static class JarSourceFileObject extends SimpleJavaFileObject {
        private final String content;
        private final String className;

        public JarSourceFileObject(String name, String content) {
            super(URI.create("jar:///" + name.replace('\\', '/')), Kind.SOURCE);
            this.content = content;
            this.className = name;
        }

        @Override
        public CharSequence getCharContent(boolean ignoreEncodingErrors) {
            return content;
        }

        @Override
        public String getName() {
            return className;
        }
    }

    /**
     * Parse Java source files and return array of ClassDescription objects.
     *
     * @param sourcePaths      Array of source file, directory paths, or JAR file
     *                         paths
     * @param classPathEntries Array of classpath entries (JARs or directories)
     * @param classPattern     Pattern to match classes (e.g., "android.app.*")
     * @param androidJar       Android boot classpath, or null for the host JDK
     * @return Array of ClassDescription objects
     */
    public ClassDescription[] parse(
            String[] sourcePaths, String[] classPathEntries, String classPattern, String androidJar)
            throws IOException {
        List<JavaFileObject> compilationUnits = new ArrayList<>();

        for (String sourcePath : sourcePaths) {
            File sourceFile = new File(sourcePath);

            if (sourceFile.isDirectory()) {
                List<File> javaFiles = new ArrayList<>();
                findJavaFiles(sourceFile, javaFiles);
                Iterable<? extends JavaFileObject> fileObjects = fileManager.getJavaFileObjectsFromFiles(javaFiles);
                for (JavaFileObject fo : fileObjects) {
                    compilationUnits.add(fo);
                }
            } else if (sourcePath.endsWith(".jar")) {
                List<JavaFileObject> jarSources = extractSourcesFromJar(sourcePath);
                compilationUnits.addAll(jarSources);
            } else if (sourcePath.endsWith(".java")) {
                JavaFileObject fileObject =
                        fileManager.getJavaFileObjects(sourceFile).iterator().next();
                compilationUnits.add(fileObject);
            }
        }

        if (compilationUnits.isEmpty()) {
            return new ClassDescription[0];
        }

        List<String> options = new ArrayList<>();
        if (classPathEntries.length > 0) {
            options.add("-classpath");
            String joinedClasspath = String.join(File.pathSeparator, classPathEntries);
            options.add(joinedClasspath);
        }
        if (androidJar != null) {
            // Android supplies its own java.* APIs, not the host JDK's named modules.
            // Source/target 8 enables a boot classpath without restricting APIs to Java SE 8.
            options.addAll(Arrays.asList("-source", "8", "-target", "8", "-bootclasspath", androidJar));
        }
        options.add("-proc:none"); // Disable annotation processing
        options.add("-nowarn"); // Suppress warnings

        if (androidJar != null) {
            compilationUnits = AndroidStubs.prepare(compiler, fileManager, compilationUnits, options);
        }

        DiagnosticCollector<JavaFileObject> diagnostics = new DiagnosticCollector<>();

        JavaCompiler.CompilationTask task = compiler.getTask(
                null, // use default writer
                fileManager,
                diagnostics,
                options,
                null, // no classes for annotation processing
                compilationUnits);

        JavacTask javacTask = (JavacTask) task;

        try {
            Iterable<? extends CompilationUnitTree> trees = javacTask.parse();
            javacTask.analyze();
            for (Diagnostic<? extends JavaFileObject> diagnostic : diagnostics.getDiagnostics()) {
                if (diagnostic.getKind() == Diagnostic.Kind.ERROR) {
                    throw new IOException(diagnostic.toString());
                }
            }

            Elements elements = javacTask.getElements();
            Types types = javacTask.getTypes();
            Trees treeUtils = Trees.instance(javacTask);

            List<ClassDescription> classes = new ArrayList<>();

            for (CompilationUnitTree tree : trees) {
                new TreePathScanner<Void, Void>() {
                    @Override
                    public Void visitClass(ClassTree node, Void p) {
                        Element element = treeUtils.getElement(getCurrentPath());
                        if (element instanceof TypeElement) {
                            TypeElement typeElement = (TypeElement) element;

                            String binaryName =
                                    elements.getBinaryName(typeElement).toString();
                            String simpleName = typeElement.getSimpleName().toString();

                            // Exclude anonymous and synthetic classes, which have no usable binding name.
                            if (isAnonymousClass(binaryName) || simpleName.isEmpty()) {
                                return super.visitClass(node, p);
                            }

                            if (classPattern == null || matchesPattern(binaryName, classPattern)) {
                                ClassDescription classDesc = analyzeClass(typeElement, elements, types);
                                if (classDesc != null) {
                                    classes.add(classDesc);
                                }
                            }
                        }
                        return super.visitClass(node, p);
                    }
                }.scan(tree, null);
            }

            return classes.toArray(new ClassDescription[0]);

        } catch (IOException e) {
            throw new IOException("Failed to parse source files: " + e.getMessage(), e);
        }
    }

    /**
     * Extract Java source files from a JAR archive.
     *
     * Extracts ALL .java files from the JAR.
     * This is necessary because the Java compiler may need access to dependency
     * classes and related types in order to properly parse and analyze the classes
     * that match the pattern. The pattern filtering happens later during class
     * analysis.
     *
     * @param jarPath Path to the JAR file
     * @return List of JavaFileObject instances for all sources in the JAR
     */
    private List<JavaFileObject> extractSourcesFromJar(String jarPath) throws IOException {
        List<JavaFileObject> sources = new ArrayList<>();

        try (JarFile jarFile = new JarFile(jarPath)) {
            Enumeration<JarEntry> entries = jarFile.entries();

            while (entries.hasMoreElements()) {
                JarEntry entry = entries.nextElement();
                String name = entry.getName();

                // Defer class-pattern filtering until after parsing the sources.
                if (!name.endsWith(".java")) {
                    continue;
                }

                try (InputStream is = jarFile.getInputStream(entry);
                        InputStreamReader reader = new InputStreamReader(is, StandardCharsets.UTF_8)) {
                    StringBuilder content = new StringBuilder();
                    char[] buffer = new char[8192];
                    int read;
                    while ((read = reader.read(buffer)) != -1) {
                        content.append(buffer, 0, read);
                    }

                    sources.add(new JarSourceFileObject(name, content.toString()));
                }
            }
        }

        return sources;
    }

    /**
     * Check if a class name represents an anonymous inner class.
     * Anonymous classes have names like: OuterClass$1, OuterClass$1$1, etc.
     * These are synthetic classes created by the compiler.
     */
    private boolean isAnonymousClass(String binaryName) {
        String[] parts = binaryName.split("\\$");
        if (parts.length <= 1) {
            return false; // No $ means not an inner class
        }

        for (int i = 1; i < parts.length; i++) {
            if (parts[i].matches("\\d+")) {
                return true;
            }
        }

        return false;
    }

    private boolean matchesPattern(String jvmClassName, String patterns) {
        if (patterns.equals("*")) {
            return true;
        }

        String[] patternList = patterns.split(",");
        for (String pattern : patternList) {
            pattern = pattern.trim();
            if (matchesSinglePattern(jvmClassName, pattern)) {
                return true;
            }
        }
        return false;
    }

    private boolean matchesSinglePattern(String jvmClassName, String pattern) {
        if (pattern.equals("*")) {
            return true;
        }

        // Pattern is given in Java format (with dots), convert to JVM format (with $)
        String jvmPattern = convertToJvmClassName(pattern);

        String regex;
        if (pattern.contains("*")) {
            regex = "^" + jvmPattern.replace(".", "\\.").replace("$", "\\$").replace("*", ".*") + "$";
        } else if (jvmPattern.contains("$")) {
            // Specific inner class pattern - match exactly
            regex = "^" + jvmPattern.replace(".", "\\.").replace("$", "\\$") + "$";
        } else {
            // Outer class pattern - also match all inner classes
            regex = "^" + jvmPattern.replace(".", "\\.").replace("$", "\\$")
                    + "(\\$.*)?$"; // Match the class itself or any inner class
        }

        return jvmClassName.matches(regex);
    }

    private ClassDescription analyzeClass(TypeElement element, Elements elements, Types types) {
        if (hasRustSkipAnnotation(element)) {
            return null;
        }

        // Local classes (and their members) can have compiler-specific captured
        // constructor parameters that the source element model does not expose.
        for (Element owner = element; owner != null; owner = owner.getEnclosingElement()) {
            if (owner instanceof TypeElement && ((TypeElement) owner).getNestingKind() == NestingKind.LOCAL) {
                throw new UnsupportedOperationException("Unsupported local class "
                        + elements.getBinaryName(element)
                        + ": captured constructor parameters require class-file input");
            }
        }

        ClassDescription desc = new ClassDescription();

        String binaryName = elements.getBinaryName(element).toString();

        desc.className = binaryName.replace('.', '/');
        desc.simpleName = element.getSimpleName().toString();

        desc.packageName = elements.getPackageOf(element).getQualifiedName().toString();

        String docComment = elements.getDocComment(element);
        desc.documentation = (docComment != null) ? docComment.trim() : "";

        desc.rustName = getRustNameAnnotation(element);

        List<MethodDescription> constructors = new ArrayList<>();
        List<MethodDescription> methods = new ArrayList<>();
        List<FieldDescription> fields = new ArrayList<>();
        List<MethodDescription> nativeMethods = new ArrayList<>();
        List<String> interfaces = new ArrayList<>();

        TypeMirror superclass = element.getSuperclass();
        if (superclass.getKind() != TypeKind.NONE) {
            String superClassName = getTypeName(superclass, types, elements);
            desc.superClass = superClassName;
        }

        for (TypeMirror interfaceType : element.getInterfaces()) {
            String interfaceName = getTypeName(interfaceType, types, elements);
            interfaces.add(interfaceName);
        }

        for (Element enclosed : element.getEnclosedElements()) {
            if (enclosed.getKind() == ElementKind.FIELD || enclosed.getKind() == ElementKind.ENUM_CONSTANT) {
                if (hasRustSkipAnnotation(enclosed)) {
                    continue;
                }
                FieldDescription field = analyzeField((VariableElement) enclosed, types, elements);
                if (field != null) {
                    fields.add(field);
                }
            } else if (enclosed.getKind() == ElementKind.CONSTRUCTOR) {
                ExecutableElement constructorElement = (ExecutableElement) enclosed;
                // Match the class-file path and ordinary methods: include non-private members.
                if (constructorElement.getModifiers().contains(Modifier.PRIVATE)) {
                    continue;
                }
                if (hasRustSkipAnnotation(constructorElement)) {
                    continue;
                }
                MethodDescription method = analyzeMethod(constructorElement, types, elements, true);
                if (method != null) {
                    constructors.add(method);
                }
            } else if (enclosed.getKind() == ElementKind.METHOD) {
                ExecutableElement methodElement = (ExecutableElement) enclosed;
                if (hasRustSkipAnnotation(methodElement)) {
                    continue;
                }
                boolean isNative = methodElement.getModifiers().contains(Modifier.NATIVE);

                MethodDescription method = analyzeMethod(methodElement, types, elements, false);
                if (method != null) {
                    if (isNative) {
                        nativeMethods.add(method);
                    } else {
                        methods.add(method);
                    }
                }
            }
        }

        desc.constructors = constructors.toArray(new MethodDescription[0]);
        desc.methods = methods.toArray(new MethodDescription[0]);
        desc.fields = fields.toArray(new FieldDescription[0]);
        desc.nativeMethods = nativeMethods.toArray(new MethodDescription[0]);
        desc.interfaces = interfaces.toArray(new String[0]);

        return desc;
    }

    private FieldDescription analyzeField(VariableElement element, Types types, Elements elements) {
        if (element.getModifiers().contains(Modifier.PRIVATE)) {
            return null;
        }

        FieldDescription desc = new FieldDescription();
        desc.name = element.getSimpleName().toString();

        String docComment = elements.getDocComment(element);
        desc.documentation = (docComment != null) ? docComment.trim() : "";

        desc.rustName = getRustNameAnnotation(element);

        TypeDescription typeDesc = createTypeDescription(element.asType(), types, elements);
        desc.typeName = typeDesc.typeName;
        desc.descriptor = typeDesc.descriptor;
        desc.arrayDimensions = typeDesc.arrayDimensions;
        desc.isPrimitive = typeDesc.isPrimitive;
        desc.isStatic = element.getModifiers().contains(Modifier.STATIC);
        desc.isFinal = element.getModifiers().contains(Modifier.FINAL);
        desc.isDeprecated = element.getAnnotation(Deprecated.class) != null;

        return desc;
    }

    private MethodDescription analyzeMethod(
            ExecutableElement element, Types types, Elements elements, boolean isConstructor) {
        boolean isPublic = element.getModifiers().contains(Modifier.PUBLIC);
        boolean isPrivate = element.getModifiers().contains(Modifier.PRIVATE);

        // Private native methods still need registration bindings.
        boolean isNative = element.getModifiers().contains(Modifier.NATIVE);
        if (isPrivate && !isNative) {
            return null;
        }

        MethodDescription desc = new MethodDescription();
        desc.name = isConstructor ? "<init>" : element.getSimpleName().toString();

        String docComment = elements.getDocComment(element);
        desc.documentation = (docComment != null) ? docComment.trim() : "";

        desc.rustName = getRustNameAnnotation(element);

        desc.isStatic = element.getModifiers().contains(Modifier.STATIC);
        desc.isConstructor = isConstructor;
        desc.isNative = isNative;
        desc.isDeprecated = element.getAnnotation(Deprecated.class) != null;
        desc.isPublic = isPublic;

        List<ParameterDescription> parameters = new ArrayList<>();

        TypeElement declaringClass = (TypeElement) element.getEnclosingElement();
        if (isConstructor
                && declaringClass.getNestingKind() == NestingKind.MEMBER
                && !declaringClass.getModifiers().contains(Modifier.STATIC)) {
            // The JVM constructor takes the immediate enclosing instance before
            // source parameters, including for implicit default constructors.
            TypeElement enclosingClass = (TypeElement) declaringClass.getEnclosingElement();
            parameters.add(createParameterDescription("enclosingInstance", enclosingClass.asType(), types, elements));
        }

        for (VariableElement param : element.getParameters()) {
            TypeMirror paramType = param.asType();

            ParameterDescription paramDesc =
                    createParameterDescription(param.getSimpleName().toString(), paramType, types, elements);
            for (AnnotationMirror annotation : param.getAnnotationMirrors()) {
                String annotationType = annotation.getAnnotationType().toString();
                if (annotationType.equals("io.github.jni_rs.jbindgen.RustPrimitive")) {
                    for (Map.Entry<? extends ExecutableElement, ? extends AnnotationValue> entry :
                            annotation.getElementValues().entrySet()) {
                        if (entry.getKey().getSimpleName().toString().equals("value")) {
                            paramDesc.rustPrimitive =
                                    entry.getValue().getValue().toString();
                            break;
                        }
                    }
                    break;
                }
            }

            parameters.add(paramDesc);
        }
        desc.parameters = parameters.toArray(new ParameterDescription[0]);

        TypeMirror returnType = element.getReturnType();
        desc.returnType = createTypeDescription(returnType, types, elements);

        return desc;
    }

    private ParameterDescription createParameterDescription(
            String name, TypeMirror type, Types types, Elements elements) {
        ParameterDescription desc = new ParameterDescription();
        TypeDescription typeDesc = createTypeDescription(type, types, elements);
        desc.name = name;
        desc.typeName = typeDesc.typeName;
        desc.descriptor = typeDesc.descriptor;
        desc.arrayDimensions = typeDesc.arrayDimensions;
        desc.isPrimitive = typeDesc.isPrimitive;
        return desc;
    }

    private String getTypeDescriptor(TypeMirror type, Types types, Elements elements) {
        switch (type.getKind()) {
            case BOOLEAN:
                return "Z";
            case BYTE:
                return "B";
            case CHAR:
                return "C";
            case SHORT:
                return "S";
            case INT:
                return "I";
            case LONG:
                return "J";
            case FLOAT:
                return "F";
            case DOUBLE:
                return "D";
            case VOID:
                return "V";
            case ARRAY:
                ArrayType arrayType = (ArrayType) type;
                return "[" + getTypeDescriptor(arrayType.getComponentType(), types, elements);
            case DECLARED:
                DeclaredType declaredType = (DeclaredType) type;
                TypeElement element = (TypeElement) declaredType.asElement();
                String binaryName = elements.getBinaryName(element).toString();
                return "L" + binaryName.replace('.', '/') + ";";
            case TYPEVAR:
                return getTypeDescriptor(types.erasure(type), types, elements);
            default:
                return "Ljava/lang/Object;";
        }
    }

    /**
     * Create a TypeDescription with proper array dimension tracking.
     */
    private TypeDescription createTypeDescription(TypeMirror type, Types types, Elements elements) {
        // JNI uses erased JVM types, including bounds and components of generic arrays.
        type = types.erasure(type);
        TypeDescription desc = new TypeDescription();

        int dimensions = 0;
        TypeMirror elementType = type;
        while (elementType.getKind() == TypeKind.ARRAY) {
            dimensions++;
            elementType = ((ArrayType) elementType).getComponentType();
        }

        desc.arrayDimensions = dimensions;
        // TypeKind.VOID.isPrimitive() returns false, but we treat void as primitive for
        // DEX signatures
        desc.isPrimitive = elementType.getKind().isPrimitive() || elementType.getKind() == TypeKind.VOID;
        desc.typeName = getTypeName(elementType, types, elements);
        desc.descriptor = getTypeDescriptor(type, types, elements);

        return desc;
    }

    private String getTypeName(TypeMirror type, Types types, Elements elements) {
        switch (type.getKind()) {
            case BOOLEAN:
                return "boolean";
            case BYTE:
                return "byte";
            case CHAR:
                return "char";
            case SHORT:
                return "short";
            case INT:
                return "int";
            case LONG:
                return "long";
            case FLOAT:
                return "float";
            case DOUBLE:
                return "double";
            case VOID:
                return "void";
            case ARRAY:
                ArrayType arrayType = (ArrayType) type;
                TypeMirror componentType = arrayType.getComponentType();
                return getTypeName(componentType, types, elements);
            case DECLARED:
                DeclaredType declaredType = (DeclaredType) type;
                TypeElement element = (TypeElement) declaredType.asElement();
                return elements.getBinaryName(element).toString();
            case TYPEVAR:
                return getTypeName(types.erasure(type), types, elements);
            default:
                return type.toString();
        }
    }

    private void findJavaFiles(File dir, List<File> javaFiles) {
        File[] files = dir.listFiles();
        if (files != null) {
            for (File file : files) {
                if (file.isDirectory()) {
                    findJavaFiles(file, javaFiles);
                } else if (file.getName().endsWith(".java")) {
                    javaFiles.add(file);
                }
            }
        }
    }

    /**
     * Extract the value from a @RustName annotation if present.
     * Returns null if the annotation is not present.
     *
     * Supports both io.github.jni_rs.jbindgen.RustName and unqualified RustName
     * for backwards compatibility.
     */
    private String getRustNameAnnotation(Element element) {
        for (AnnotationMirror annotation : element.getAnnotationMirrors()) {
            String annotationName = annotation.getAnnotationType().toString();
            if (annotationName.equals("io.github.jni_rs.jbindgen.RustName")
                    || annotationName.equals("RustName")
                    || annotationName.endsWith(".RustName")) {
                for (Map.Entry<? extends ExecutableElement, ? extends AnnotationValue> entry :
                        annotation.getElementValues().entrySet()) {
                    if (entry.getKey().getSimpleName().toString().equals("value")) {
                        String value = entry.getValue().toString();
                        if (value.startsWith("\"") && value.endsWith("\"")) {
                            return value.substring(1, value.length() - 1);
                        }
                        return value;
                    }
                }
            }
        }
        return null;
    }

    /**
     * Check if an element has a @RustSkip annotation.
     * Returns true if the annotation is present, indicating the element should be
     * skipped.
     *
     * Supports both io.github.jni_rs.jbindgen.RustSkip and unqualified RustSkip
     * for backwards compatibility.
     */
    private boolean hasRustSkipAnnotation(Element element) {
        for (AnnotationMirror annotation : element.getAnnotationMirrors()) {
            String annotationName = annotation.getAnnotationType().toString();
            if (annotationName.equals("io.github.jni_rs.jbindgen.RustSkip")
                    || annotationName.equals("RustSkip")
                    || annotationName.endsWith(".RustSkip")) {
                return true;
            }
        }
        return false;
    }

    // JNI accesses these data classes, so they must be public static.
    public static class ClassDescription {
        public String className;
        public String packageName;
        public String simpleName;
        public String documentation; // Javadoc comment for the class
        public String rustName; // Optional Rust type name from @RustName annotation
        public MethodDescription[] constructors;
        public MethodDescription[] methods;
        public FieldDescription[] fields;
        public MethodDescription[] nativeMethods;
        // Interfaces have no superclass; keep absence distinct from a Java null string.
        public String superClass = "";
        public String[] interfaces;
    }

    public static class MethodDescription {
        public String name;
        public String documentation; // Javadoc comment for the method/constructor
        public String rustName; // Optional Rust method name from @RustName annotation
        public boolean isStatic;
        public boolean isConstructor;
        public boolean isNative;
        public boolean isDeprecated;
        public boolean isPublic;
        public ParameterDescription[] parameters;
        public TypeDescription returnType;
    }

    public static class ParameterDescription {
        public String name;
        public String typeName; // Element type name (without [] suffix)
        public String descriptor; // JNI descriptor
        public int arrayDimensions; // 0 for non-array, 1 for T[], 2 for T[][], etc.
        public boolean isPrimitive; // true for primitive element types
        public String rustPrimitive; // Rust type name from @RustPrimitive annotation (may be null)
    }

    public static class TypeDescription {
        public String typeName; // Element type name (without [] suffix)
        public String descriptor; // JNI descriptor
        public int arrayDimensions; // 0 for non-array, 1 for T[], 2 for T[][], etc.
        public boolean isPrimitive; // true for primitive types (int, boolean, etc.)
    }

    public static class FieldDescription {
        public String name;
        public String documentation; // Javadoc comment for the field
        public String rustName; // Optional Rust field name from @RustName annotation
        public String typeName; // Element type name (without [] suffix)
        public String descriptor; // JNI descriptor
        public int arrayDimensions; // 0 for non-array, 1 for T[], 2 for T[][], etc.
        public boolean isPrimitive; // true for primitive element types
        public boolean isStatic;
        public boolean isFinal;
        public boolean isDeprecated;
    }

    /**
     * Convert a Java qualified name to JVM class name format.
     * Inner classes use $ separator instead of . separator.
     * For example: android.os.Build.Partition -> android.os.Build$Partition
     *
     * Uses a heuristic: parts that start with uppercase are likely class names.
     * The first uppercase part is the outer class, subsequent uppercase parts are
     * inner classes.
     */
    private String convertToJvmClassName(String qualifiedName) {
        String[] parts = qualifiedName.split("\\.");

        StringBuilder result = new StringBuilder();
        boolean foundFirstClass = false;

        for (int i = 0; i < parts.length; i++) {
            if (i > 0) {
                if (foundFirstClass && parts[i].length() > 0 && Character.isUpperCase(parts[i].charAt(0))) {
                    result.append('$');
                } else {
                    result.append('.');
                }
            }

            result.append(parts[i]);

            if (!foundFirstClass && parts[i].length() > 0 && Character.isUpperCase(parts[i].charAt(0))) {
                foundFirstClass = true;
            }
        }

        return result.toString();
    }
}
