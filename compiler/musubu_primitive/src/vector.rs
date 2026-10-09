use super::*;

impl ToPrimitiveType for Vector {
    fn to_type(&self) -> PrimitiveType {
        let (dimension, element_type) = match self {
            Self::Components {
                elements,
                element_type,
            } => (elements.len() as u32, element_type.clone()),
            Self::Vector3(_) => (3, PrimitiveType::default_float()),
            Self::Vector4(_) => (4, PrimitiveType::default_float()),
        };
        PrimitiveType::Vector {
            dimension,
            type_kind: Box::new(element_type),
        }
    }
}

impl Vector {
    pub fn components(&self) -> Vec<Value> {
        match self {
            Self::Components { elements, .. } => elements.clone(),
            Self::Vector3(vector) => vector
                .iter()
                .map(|v| Value::Float(Float::Float32(*v)))
                .collect(),
            Self::Vector4(vector) => vector
                .iter()
                .map(|v| Value::Float(Float::Float32(*v)))
                .collect(),
        }
    }

    pub fn components_mut(&mut self) -> &mut [Value] {
        // Normalize legacy host values before updating individual components.
        if !matches!(self, Self::Components { .. }) {
            *self = Self::Components {
                elements: self.components(),
                element_type: PrimitiveType::default_float(),
            };
        }
        let Self::Components { elements, .. } = self else {
            unreachable!();
        };
        elements
    }
}
