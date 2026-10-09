import org.jetbrains.intellij.platform.gradle.TestFrameworkType
import org.jetbrains.intellij.platform.gradle.tasks.VerifyPluginTask
import org.gradle.api.GradleException
import org.gradle.api.tasks.compile.JavaCompile
import org.gradle.api.tasks.testing.logging.TestExceptionFormat
import org.gradle.api.tasks.testing.logging.TestLogEvent
import org.gradle.jvm.toolchain.JavaLanguageVersion

plugins {
    java
    id("org.jetbrains.intellij.platform") version "2.19.0"
}

group = "com.disnana.nagi"
version = "0.1.3"

repositories {
    mavenCentral()
    intellijPlatform { defaultRepositories() }
}

dependencies {
    intellijPlatform {
        val localPath = providers.gradleProperty("localPlatformPath").orNull
        if (localPath != null) local(localPath)
        else create(
            providers.gradleProperty("platformType").getOrElse("IC"),
            providers.gradleProperty("platformVersion").getOrElse("2025.1.1"),
        ) { useInstaller.set(false) }
        testFramework(TestFrameworkType.Platform)
        pluginVerifier()
    }
    testImplementation("junit:junit:4.13.2")
}

java {
    toolchain {
        languageVersion.set(providers.gradleProperty("nagiJavaToolchainVersion")
            .map { it.toInt() }
            .orElse(21)
            .map { JavaLanguageVersion.of(it) })
    }
    sourceCompatibility = JavaVersion.VERSION_21
    targetCompatibility = JavaVersion.VERSION_21
}

val expectedNagiCompilerVersion = providers.gradleProperty("nagiJavaToolchainVersion")
    .map { it.toInt() }
    .orElse(21)

val verifyNagiJavaToolchain = tasks.register("verifyNagiJavaToolchain") {
    val report = layout.buildDirectory.file("verification-metadata/nagi-java-toolchain.json")
    inputs.property("expectedNagiCompilerVersion", expectedNagiCompilerVersion)
    outputs.file(report)
    outputs.upToDateWhen { false }
    doLast {
        val expectedVersion = expectedNagiCompilerVersion.get()
        if (expectedVersion !in setOf(21, 25)) {
            throw GradleException("Supported Nagi IDE compiler toolchains are 21 and 25, got $expectedVersion")
        }
        val compilerRecords = tasks.withType<JavaCompile>().map { compileTask ->
            val selectedCompiler = compileTask.javaCompiler.orNull
                ?: throw GradleException("Gradle did not select a Java compiler for ${compileTask.name}")
            val actualVersion = selectedCompiler.metadata.languageVersion.asInt()
            val release = compileTask.options.release.orNull
            val source = compileTask.sourceCompatibility
            val target = compileTask.targetCompatibility
            logger.lifecycle(
                "Nagi Java compiler toolchain: task=${compileTask.name} expected=$expectedVersion " +
                    "selected=$actualVersion source=$source target=$target release=$release"
            )
            if (actualVersion != expectedVersion) {
                throw GradleException(
                    "Expected javac toolchain $expectedVersion for ${compileTask.name}, but Gradle selected $actualVersion"
                )
            }
            if (source != "21" || target != "21" || release != 21) {
                throw GradleException(
                    "The common Nagi plugin must keep Java 21 bytecode: task=${compileTask.name} " +
                        "source=$source target=$target release=$release"
                )
            }
            mapOf(
                "task" to compileTask.name,
                "selectedCompilerMajor" to actualVersion,
                "sourceCompatibility" to source,
                "targetCompatibility" to target,
                "release" to release,
            )
        }
        if (compilerRecords.isEmpty()) throw GradleException("No JavaCompile task was configured for the Nagi plugin")
        val mainCompile = compilerRecords.firstOrNull { it["task"] == "compileJava" }
            ?: throw GradleException("Gradle did not configure the compileJava task")
        val destination = report.get().asFile
        destination.parentFile.mkdirs()
        destination.writeText(
            buildString {
                appendLine("{")
                appendLine("  \"expectedCompilerMajor\": $expectedVersion,")
                appendLine("  \"selectedCompilerMajor\": ${mainCompile["selectedCompilerMajor"]},")
                appendLine("  \"sourceCompatibility\": \"${mainCompile["sourceCompatibility"]}\",")
                appendLine("  \"targetCompatibility\": \"${mainCompile["targetCompatibility"]}\",")
                appendLine("  \"release\": ${mainCompile["release"]},")
                appendLine("  \"compileTasks\": [")
                compilerRecords.forEachIndexed { index, record ->
                    val comma = if (index + 1 < compilerRecords.size) "," else ""
                    appendLine(
                        "    {\"task\": \"${record["task"]}\", " +
                            "\"selectedCompilerMajor\": ${record["selectedCompilerMajor"]}, " +
                            "\"release\": ${record["release"]}}$comma"
                    )
                }
                appendLine("  ]")
                appendLine("}")
            },
            Charsets.UTF_8,
        )
    }
}

tasks.withType<JavaCompile>().configureEach {
    options.release.set(21)
    dependsOn(verifyNagiJavaToolchain)
}

intellijPlatform {
    pluginConfiguration {
        ideaVersion {
            sinceBuild = "251.25410.109"
        }
    }
    pluginVerification {
        ides {
            val localPath = providers.gradleProperty("localPlatformPath").orNull
            if (localPath != null) local(localPath)
            else {
                val type = providers.gradleProperty("platformType").getOrElse("IC")
                val version = providers.gradleProperty("platformVersion").getOrElse("2025.1.1")
                create(type, version) { useInstaller.set(false) }
                val minimum = providers.gradleProperty("minimumPlatformVersion").orNull
                if (minimum != null && minimum != version) {
                    create(type, minimum) { useInstaller.set(false) }
                }
            }
        }
    }
}

tasks.named<VerifyPluginTask>("verifyPlugin") {
    freeArgs.addAll(listOf("-mute", "TemplateWordInPluginName"))
}

providers.gradleProperty("verificationArchive").orNull?.let { archivePath ->
    tasks.named<VerifyPluginTask>("verifyPlugin") {
        archiveFile.set(file(archivePath))
    }
}

tasks.test {
    systemProperty("java.awt.headless", "true")
    // EAP 263 otherwise treats all headless/unit-test projects as trusted.
    // Exercise the real trust gate; this property is limited to the test JVM.
    systemProperty("idea.trust.headless.disabled", "false")
    testLogging {
        events = setOf(TestLogEvent.PASSED, TestLogEvent.FAILED, TestLogEvent.SKIPPED)
        exceptionFormat = TestExceptionFormat.FULL
        showCauses = true
        showExceptions = true
        showStackTraces = true
    }
}
