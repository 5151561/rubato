// 差分垫片:androidx.room 注解(真身实体按原路径挂载,注解只需能编译)
@file:Suppress("unused")

package androidx.room

import kotlin.reflect.KClass

@Target(AnnotationTarget.CLASS)
annotation class Entity(
    val tableName: String = "",
    val indices: Array<Index> = [],
    val inheritSuperIndices: Boolean = false,
    val primaryKeys: Array<String> = [],
    val foreignKeys: Array<ForeignKey> = [],
    val ignoredColumns: Array<String> = [],
)

@Target(allowedTargets = [])
annotation class Index(val value: Array<String> = [], val unique: Boolean = false)

@Target(allowedTargets = [])
annotation class ForeignKey(
    val entity: KClass<*>,
    val parentColumns: Array<String>,
    val childColumns: Array<String>,
    val onDelete: Int = 1,
    val onUpdate: Int = 1,
    val deferred: Boolean = false,
) {
    companion object {
        const val NO_ACTION = 1
        const val RESTRICT = 2
        const val SET_NULL = 3
        const val SET_DEFAULT = 4
        const val CASCADE = 5
    }
}

@Target(AnnotationTarget.FIELD, AnnotationTarget.VALUE_PARAMETER, AnnotationTarget.PROPERTY)
annotation class ColumnInfo(
    val name: String = "[field-name]",
    val typeAffinity: Int = 1,
    val index: Boolean = false,
    val collate: Int = 1,
    val defaultValue: String = "[value-unspecified]",
)

@Target(
    AnnotationTarget.FIELD,
    AnnotationTarget.VALUE_PARAMETER,
    AnnotationTarget.PROPERTY,
    AnnotationTarget.CONSTRUCTOR,
)
annotation class PrimaryKey(val autoGenerate: Boolean = false)

@Target(
    AnnotationTarget.FIELD,
    AnnotationTarget.VALUE_PARAMETER,
    AnnotationTarget.PROPERTY,
    AnnotationTarget.PROPERTY_GETTER,
    AnnotationTarget.PROPERTY_SETTER,
    AnnotationTarget.CONSTRUCTOR,
    AnnotationTarget.FUNCTION,
)
annotation class Ignore

@Target(AnnotationTarget.FUNCTION, AnnotationTarget.PROPERTY_GETTER, AnnotationTarget.PROPERTY_SETTER)
annotation class TypeConverter

@Target(
    AnnotationTarget.CLASS,
    AnnotationTarget.FIELD,
    AnnotationTarget.VALUE_PARAMETER,
    AnnotationTarget.PROPERTY,
    AnnotationTarget.FUNCTION,
)
annotation class TypeConverters(vararg val value: KClass<*>)

@Target(AnnotationTarget.CLASS)
annotation class DatabaseView(val value: String = "", val viewName: String = "")
