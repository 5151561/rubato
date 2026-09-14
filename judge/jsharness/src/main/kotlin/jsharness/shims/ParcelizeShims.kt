// 差分垫片:kotlinx.parcelize 注解(无编译器插件时只是普通注解,足够编译)
@file:Suppress("unused")

package kotlinx.parcelize

@Target(AnnotationTarget.CLASS)
annotation class Parcelize

@Target(AnnotationTarget.PROPERTY)
annotation class IgnoredOnParcel
